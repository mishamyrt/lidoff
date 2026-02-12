//
//  external_display_gamma.m
//  lidoff - external display DDC + gamma dimming
//

#import "external_display_backend.h"
#import <CoreGraphics/CoreGraphics.h>
#import <CoreGraphics/CGDisplayConfiguration.h>
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
static size_t displayBackupCapacity = 0;

static void resetBackup(DisplayBackup *backup) {
    if (!backup) {
        return;
    }
    free(backup->gammaRed);
    free(backup->gammaGreen);
    free(backup->gammaBlue);
    backup->gammaRed = NULL;
    backup->gammaGreen = NULL;
    backup->gammaBlue = NULL;
    backup->gammaSampleCount = 0;
}

static void clearBackups(void) {
    if (!displayBackups) {
        displayBackupCount = 0;
        displayBackupCapacity = 0;
        return;
    }
    
    for (size_t i = 0; i < displayBackupCount; i++) {
        resetBackup(&displayBackups[i]);
    }
    
    free(displayBackups);
    displayBackups = NULL;
    displayBackupCount = 0;
    displayBackupCapacity = 0;
}

void ExternalDisplayGammaClearBackups(void) {
    clearBackups();
}

BOOL ExternalDisplayGammaPrepare(size_t displayCount) {
    clearBackups();
    if (displayCount == 0) {
        return YES;
    }
    
    displayBackups = calloc(displayCount, sizeof(DisplayBackup));
    if (!displayBackups) {
        return NO;
    }
    
    displayBackupCapacity = displayCount;
    return YES;
}

void ExternalDisplayGammaFinalize(void) {
    if (displayBackupCount == 0) {
        clearBackups();
    }
}

BOOL ExternalDisplayGammaHasBackups(void) {
    return displayBackupCount > 0;
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

static BOOL restoreDisplayFromBackup(const DisplayBackup *backup) {
    if (!backup) {
        return NO;
    }
    
    if (!CGDisplayIsOnline(backup->displayID)) {
        return NO;
    }
    
    if (backup->gammaSampleCount > 0 &&
        backup->gammaRed &&
        backup->gammaGreen &&
        backup->gammaBlue) {
        CGSetDisplayTransferByTable(
            backup->displayID,
            backup->gammaSampleCount,
            backup->gammaRed,
            backup->gammaGreen,
            backup->gammaBlue
        );
    }
    
    io_service_t framebuffer = CGDisplayIOServicePort(backup->displayID);
    if (framebuffer == MACH_PORT_NULL) {
        return YES;
    }
    if (backup->hasBrightness) {
        ddcSetVCP(framebuffer, 0x10, backup->brightness);
    }
    if (backup->hasContrast) {
        ddcSetVCP(framebuffer, 0x12, backup->contrast);
    }
    
    return YES;
}

BOOL ExternalDisplayGammaDisableDisplay(CGDirectDisplayID displayID) {
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
    
    if (!displayBackups || displayBackupCount >= displayBackupCapacity) {
        restoreDisplayFromBackup(&backup);
        free(backup.gammaRed);
        free(backup.gammaGreen);
        free(backup.gammaBlue);
        return NO;
    }
    
    displayBackups[displayBackupCount++] = backup;
    return YES;
}

size_t ExternalDisplayGammaRestoreAll(void) {
    if (!displayBackups || displayBackupCount == 0) {
        return 0;
    }

    size_t restored = 0;
    size_t remaining = 0;
    for (size_t i = 0; i < displayBackupCount; i++) {
        DisplayBackup backup = displayBackups[i];
        if (restoreDisplayFromBackup(&backup)) {
            restored++;
            resetBackup(&backup);
            continue;
        }
        displayBackups[remaining++] = backup;
    }

    displayBackupCount = remaining;
    if (displayBackupCount == 0) {
        clearBackups();
    }

    return restored;
}

static NSDictionary *copyGammaState(void) {
    if (!displayBackups || displayBackupCount == 0) {
        return nil;
    }
    
    NSMutableArray *backups = [NSMutableArray arrayWithCapacity:displayBackupCount];
    for (size_t i = 0; i < displayBackupCount; i++) {
        const DisplayBackup *backup = &displayBackups[i];
        NSMutableDictionary *entry = [NSMutableDictionary dictionary];
        entry[@"displayID"] = @(backup->displayID);
        entry[@"brightness"] = @(backup->brightness);
        entry[@"contrast"] = @(backup->contrast);
        entry[@"hasBrightness"] = @(backup->hasBrightness);
        entry[@"hasContrast"] = @(backup->hasContrast);
        entry[@"gammaSampleCount"] = @(backup->gammaSampleCount);
        
        if (backup->gammaSampleCount > 0 &&
            backup->gammaRed &&
            backup->gammaGreen &&
            backup->gammaBlue) {
            size_t length = (size_t)backup->gammaSampleCount * sizeof(float);
            entry[@"gammaRed"] = [NSData dataWithBytes:backup->gammaRed length:length];
            entry[@"gammaGreen"] = [NSData dataWithBytes:backup->gammaGreen length:length];
            entry[@"gammaBlue"] = [NSData dataWithBytes:backup->gammaBlue length:length];
        }
        
        [backups addObject:entry];
    }
    
    return @{@"backups": backups};
}

static float *copyGammaTable(NSData *data, uint32_t sampleCount) {
    if (!data || sampleCount == 0) {
        return NULL;
    }
    
    size_t expectedLength = (size_t)sampleCount * sizeof(float);
    if (data.length < expectedLength) {
        return NULL;
    }
    
    float *table = calloc(sampleCount, sizeof(float));
    if (!table) {
        return NULL;
    }
    
    memcpy(table, data.bytes, expectedLength);
    return table;
}

static DisplayBackup gammaBackupFromState(NSDictionary *entry, BOOL *validOut) {
    DisplayBackup backup = {
        .displayID = 0,
        .brightness = 0,
        .contrast = 0,
        .hasBrightness = NO,
        .hasContrast = NO,
        .gammaSampleCount = 0,
        .gammaRed = NULL,
        .gammaGreen = NULL,
        .gammaBlue = NULL
    };
    
    NSNumber *displayIDValue = entry[@"displayID"];
    if (![displayIDValue isKindOfClass:[NSNumber class]]) {
        if (validOut) {
            *validOut = NO;
        }
        return backup;
    }
    
    backup.displayID = (CGDirectDisplayID)displayIDValue.unsignedIntValue;
    backup.hasBrightness = [entry[@"hasBrightness"] boolValue];
    backup.hasContrast = [entry[@"hasContrast"] boolValue];
    backup.brightness = (uint16_t)[entry[@"brightness"] unsignedIntValue];
    backup.contrast = (uint16_t)[entry[@"contrast"] unsignedIntValue];
    backup.gammaSampleCount = (uint32_t)[entry[@"gammaSampleCount"] unsignedIntValue];
    
    if (backup.gammaSampleCount > 0) {
        NSData *red = entry[@"gammaRed"];
        NSData *green = entry[@"gammaGreen"];
        NSData *blue = entry[@"gammaBlue"];
        backup.gammaRed = copyGammaTable(red, backup.gammaSampleCount);
        backup.gammaGreen = copyGammaTable(green, backup.gammaSampleCount);
        backup.gammaBlue = copyGammaTable(blue, backup.gammaSampleCount);
        if (!backup.gammaRed || !backup.gammaGreen || !backup.gammaBlue) {
            free(backup.gammaRed);
            free(backup.gammaGreen);
            free(backup.gammaBlue);
            backup.gammaRed = NULL;
            backup.gammaGreen = NULL;
            backup.gammaBlue = NULL;
            backup.gammaSampleCount = 0;
        }
    }
    
    if (validOut) {
        *validOut = (backup.displayID != 0);
    }
    return backup;
}

static BOOL restoreGammaFromState(NSDictionary *state, size_t *restoredCountOut) {
    NSArray *backups = state[@"backups"];
    if (![backups isKindOfClass:[NSArray class]]) {
        return NO;
    }
    
    size_t restored = 0;
    for (NSDictionary *entry in backups) {
        if (![entry isKindOfClass:[NSDictionary class]]) {
            continue;
        }
        
        BOOL valid = NO;
        DisplayBackup backup = gammaBackupFromState(entry, &valid);
        if (!valid) {
            continue;
        }
        
        if (restoreDisplayFromBackup(&backup)) {
            restored++;
        }
        
        resetBackup(&backup);
    }
    
    if (restoredCountOut) {
        *restoredCountOut = restored;
    }
    
    clearBackups();
    return YES;
}

const ExternalDisplayBackend *ExternalDisplayBackendGamma(void) {
    static const ExternalDisplayBackend backend = {
        .name = "gamma",
        .prepare = ExternalDisplayGammaPrepare,
        .finalize = ExternalDisplayGammaFinalize,
        .clearBackups = ExternalDisplayGammaClearBackups,
        .disableDisplay = ExternalDisplayGammaDisableDisplay,
        .restoreAll = ExternalDisplayGammaRestoreAll,
        .hasBackups = ExternalDisplayGammaHasBackups,
        .copyState = copyGammaState,
        .restoreFromState = restoreGammaFromState
    };
    return &backend;
}
