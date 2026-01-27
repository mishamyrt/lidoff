//
//  external_display.m
//  lidoff - external display DDC + gamma dimming
//

#import "external_display.h"
#import <CoreGraphics/CoreGraphics.h>
#import <IOKit/IOKitLib.h>
#import <IOKit/i2c/IOI2CInterface.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

#define DDC_ADDRESS 0x37
#define DDC_HEADER 0x51
#define DDC_CMD_SET_VCP 0x03
#define DDC_CMD_GET_VCP 0x01
#define DDC_LEN_SET_VCP 0x84
#define DDC_LEN_GET_VCP 0x82

typedef struct {
    CGDirectDisplayID displayID;
    uint16_t brightness;
    uint16_t contrast;
    BOOL hasBrightness;
    BOOL hasContrast;
    uint32_t gammaSampleCount;
    float *gammaRed;
    float *gammaGreen;
    float *gammaBlue;
} DisplayBackup;

static DisplayBackup *displayBackups = NULL;
static size_t displayBackupCount = 0;
static BOOL externalDisplaysDisabled = NO;

static void clearBackups(void) {
    if (!displayBackups) {
        displayBackupCount = 0;
        return;
    }
    
    for (size_t i = 0; i < displayBackupCount; i++) {
        free(displayBackups[i].gammaRed);
        free(displayBackups[i].gammaGreen);
        free(displayBackups[i].gammaBlue);
    }
    
    free(displayBackups);
    displayBackups = NULL;
    displayBackupCount = 0;
}

static uint8_t ddcChecksum(const uint8_t *payload, size_t length) {
    uint8_t checksum = (uint8_t)(DDC_ADDRESS << 1);
    for (size_t i = 0; i < length; i++) {
        checksum ^= payload[i];
    }
    return checksum;
}

static IOReturn sendI2CRequest(io_service_t framebuffer, IOI2CRequest *request) {
    io_service_t interface = MACH_PORT_NULL;
    IOReturn status = IOFBCopyI2CInterfaceForBus(framebuffer, 0, &interface);
    if (status != kIOReturnSuccess || interface == MACH_PORT_NULL) {
        return status;
    }
    
    IOI2CConnectRef connect = NULL;
    status = IOI2CInterfaceOpen(interface, kNilOptions, &connect);
    if (status == kIOReturnSuccess) {
        status = IOI2CSendRequest(connect, kNilOptions, request);
        IOI2CInterfaceClose(connect, kNilOptions);
        if (status == kIOReturnSuccess) {
            status = request->result;
        }
    }
    
    IOObjectRelease(interface);
    return status;
}

static BOOL ddcGetVCP(io_service_t framebuffer, uint8_t code, uint16_t *valueOut) {
    uint8_t sendBuffer[5] = {DDC_HEADER, DDC_LEN_GET_VCP, DDC_CMD_GET_VCP, code, 0};
    sendBuffer[4] = ddcChecksum(sendBuffer, 4);
    
    uint8_t replyBuffer[11] = {0};
    
    IOI2CRequest request;
    memset(&request, 0, sizeof(request));
    request.sendTransactionType = kIOI2CSimpleTransactionType;
    request.replyTransactionType = kIOI2CDDCciReplyTransactionType;
    request.sendAddress = DDC_ADDRESS << 1;
    request.replyAddress = DDC_ADDRESS << 1;
    request.sendBytes = (uint32_t)sizeof(sendBuffer);
    request.replyBytes = (uint32_t)sizeof(replyBuffer);
    request.sendBuffer = (vm_address_t)sendBuffer;
    request.replyBuffer = (vm_address_t)replyBuffer;
    
    IOReturn status = sendI2CRequest(framebuffer, &request);
    if (status != kIOReturnSuccess) {
        return NO;
    }
    
    if (replyBuffer[2] != 0x02 || replyBuffer[4] != code) {
        return NO;
    }
    
    uint16_t value = (uint16_t)((replyBuffer[8] << 8) | replyBuffer[9]);
    *valueOut = value;
    return YES;
}

static BOOL ddcSetVCP(io_service_t framebuffer, uint8_t code, uint16_t value) {
    uint8_t sendBuffer[7] = {
        DDC_HEADER,
        DDC_LEN_SET_VCP,
        DDC_CMD_SET_VCP,
        code,
        (uint8_t)(value >> 8),
        (uint8_t)(value & 0xFF),
        0
    };
    sendBuffer[6] = ddcChecksum(sendBuffer, 6);
    
    IOI2CRequest request;
    memset(&request, 0, sizeof(request));
    request.sendTransactionType = kIOI2CSimpleTransactionType;
    request.replyTransactionType = kIOI2CNoTransactionType;
    request.sendAddress = DDC_ADDRESS << 1;
    request.sendBytes = (uint32_t)sizeof(sendBuffer);
    request.sendBuffer = (vm_address_t)sendBuffer;
    
    IOReturn status = sendI2CRequest(framebuffer, &request);
    return (status == kIOReturnSuccess);
}

static BOOL backupAndZeroGamma(CGDirectDisplayID displayID, DisplayBackup *backupOut) {
    size_t capacity = CGDisplayGammaTableCapacity(displayID);
    if (capacity == 0 || capacity > UINT32_MAX) {
        return NO;
    }
    
    float *red = calloc(capacity, sizeof(float));
    float *green = calloc(capacity, sizeof(float));
    float *blue = calloc(capacity, sizeof(float));
    uint32_t sampleCount = 0;
    
    CGError err = CGGetDisplayTransferByTable(
        displayID,
        (uint32_t)capacity,
        red,
        green,
        blue,
        &sampleCount
    );
    if (err != kCGErrorSuccess || sampleCount == 0) {
        free(red);
        free(green);
        free(blue);
        return NO;
    }
    
    float *zeros = calloc((size_t)sampleCount, sizeof(float));
    CGSetDisplayTransferByTable(displayID, sampleCount, zeros, zeros, zeros);
    free(zeros);
    
    backupOut->gammaSampleCount = sampleCount;
    backupOut->gammaRed = red;
    backupOut->gammaGreen = green;
    backupOut->gammaBlue = blue;
    return YES;
}

BOOL ExternalDisplaysDisable(void) {
    if (externalDisplaysDisabled) {
        return YES;
    }
    
    clearBackups();
    
    CGDirectDisplayID displays[32];
    CGDisplayCount displayCount = 0;
    CGError err = CGGetOnlineDisplayList(32, displays, &displayCount);
    if (err != kCGErrorSuccess) {
        return NO;
    }
    
    DisplayBackup *newBackups = calloc(displayCount, sizeof(DisplayBackup));
    size_t newCount = 0;
    
    for (CGDisplayCount i = 0; i < displayCount; i++) {
        CGDirectDisplayID displayID = displays[i];
        if (CGDisplayIsBuiltin(displayID)) {
            continue;
        }
        
        DisplayBackup backup = {
            .displayID = displayID,
            .brightness = 0,
            .contrast = 0,
            .hasBrightness = NO,
            .hasContrast = NO,
            .gammaSampleCount = 0,
            .gammaRed = NULL,
            .gammaGreen = NULL,
            .gammaBlue = NULL
        };
        
        io_service_t framebuffer = CGDisplayIOServicePort(displayID);
        if (framebuffer != MACH_PORT_NULL) {
            uint16_t value = 0;
            if (ddcGetVCP(framebuffer, 0x10, &value)) {
                backup.brightness = value;
                backup.hasBrightness = YES;
                ddcSetVCP(framebuffer, 0x10, 0);
            }
            if (ddcGetVCP(framebuffer, 0x12, &value)) {
                backup.contrast = value;
                backup.hasContrast = YES;
                ddcSetVCP(framebuffer, 0x12, 0);
            }
        }
        
        backupAndZeroGamma(displayID, &backup);
        
        newBackups[newCount] = backup;
        newCount++;
    }
    
    if (newCount == 0) {
        free(newBackups);
        externalDisplaysDisabled = NO;
        return YES;
    }
    
    displayBackups = newBackups;
    displayBackupCount = newCount;
    externalDisplaysDisabled = YES;
    return YES;
}

BOOL ExternalDisplaysRestore(void) {
    if (!externalDisplaysDisabled && displayBackupCount == 0) {
        return YES;
    }
    
    for (size_t i = 0; i < displayBackupCount; i++) {
        if (displayBackups[i].gammaSampleCount > 0 &&
            displayBackups[i].gammaRed &&
            displayBackups[i].gammaGreen &&
            displayBackups[i].gammaBlue) {
            CGSetDisplayTransferByTable(
                displayBackups[i].displayID,
                displayBackups[i].gammaSampleCount,
                displayBackups[i].gammaRed,
                displayBackups[i].gammaGreen,
                displayBackups[i].gammaBlue
            );
        }
        
        io_service_t framebuffer = CGDisplayIOServicePort(displayBackups[i].displayID);
        if (framebuffer == MACH_PORT_NULL) {
            continue;
        }
        if (displayBackups[i].hasBrightness) {
            ddcSetVCP(framebuffer, 0x10, displayBackups[i].brightness);
        }
        if (displayBackups[i].hasContrast) {
            ddcSetVCP(framebuffer, 0x12, displayBackups[i].contrast);
        }
    }
    
    clearBackups();
    externalDisplaysDisabled = NO;
    return YES;
}

BOOL ExternalDisplaysAreDisabled(void) {
    return externalDisplaysDisabled;
}
