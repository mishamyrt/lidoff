//
//  lid_sensor.m
//  lidoff - MacBook lid angle HID sensor
//

#import "lid_sensor.h"
#import <IOKit/hid/IOHIDManager.h>
#import <IOKit/hid/IOHIDDevice.h>

static IOHIDManagerRef hidManager = NULL;
static IOHIDDeviceRef lidDevice = NULL;

static BOOL isLidAngleSensor(IOHIDDeviceRef device) {
    CFNumberRef vidRef = IOHIDDeviceGetProperty(device, CFSTR(kIOHIDVendorIDKey));
    if (!vidRef) return NO;
    
    int vid = 0;
    CFNumberGetValue(vidRef, kCFNumberIntType, &vid);
    if (vid != LID_SENSOR_VID) return NO;
    
    CFNumberRef pidRef = IOHIDDeviceGetProperty(device, CFSTR(kIOHIDProductIDKey));
    if (!pidRef) return NO;
    
    int pid = 0;
    CFNumberGetValue(pidRef, kCFNumberIntType, &pid);
    if (pid != LID_SENSOR_PID) return NO;
    
    CFNumberRef usagePageRef = IOHIDDeviceGetProperty(device, CFSTR(kIOHIDPrimaryUsagePageKey));
    if (!usagePageRef) return NO;
    
    int usagePage = 0;
    CFNumberGetValue(usagePageRef, kCFNumberIntType, &usagePage);
    if (usagePage != LID_SENSOR_USAGE_PAGE) return NO;
    
    CFNumberRef usageRef = IOHIDDeviceGetProperty(device, CFSTR(kIOHIDPrimaryUsageKey));
    if (!usageRef) return NO;
    
    int usage = 0;
    CFNumberGetValue(usageRef, kCFNumberIntType, &usage);
    if (usage != LID_SENSOR_USAGE) return NO;
    
    return YES;
}

static IOHIDDeviceRef findLidSensorDevice(IOHIDManagerRef manager) {
    CFSetRef deviceSet = IOHIDManagerCopyDevices(manager);
    if (!deviceSet) return NULL;
    
    CFIndex count = CFSetGetCount(deviceSet);
    if (count == 0) {
        CFRelease(deviceSet);
        return NULL;
    }
    
    IOHIDDeviceRef *devices = malloc(sizeof(IOHIDDeviceRef) * count);
    CFSetGetValues(deviceSet, (const void **)devices);
    
    IOHIDDeviceRef foundDevice = NULL;
    for (CFIndex i = 0; i < count; i++) {
        if (isLidAngleSensor(devices[i])) {
            foundDevice = devices[i];
            CFRetain(foundDevice);
            break;
        }
    }
    
    free(devices);
    CFRelease(deviceSet);
    
    return foundDevice;
}

BOOL LidSensorInit(void) {
    hidManager = IOHIDManagerCreate(kCFAllocatorDefault, kIOHIDOptionsTypeNone);
    if (!hidManager) {
        return NO;
    }
    
    NSDictionary *matching = @{
        @(kIOHIDVendorIDKey): @(LID_SENSOR_VID),
        @(kIOHIDProductIDKey): @(LID_SENSOR_PID)
    };
    
    IOHIDManagerSetDeviceMatching(hidManager, (__bridge CFDictionaryRef)matching);
    
    IOReturn result = IOHIDManagerOpen(hidManager, kIOHIDOptionsTypeNone);
    if (result != kIOReturnSuccess) {
        CFRelease(hidManager);
        hidManager = NULL;
        return NO;
    }
    
    lidDevice = findLidSensorDevice(hidManager);
    if (!lidDevice) {
        IOHIDManagerClose(hidManager, kIOHIDOptionsTypeNone);
        CFRelease(hidManager);
        hidManager = NULL;
        return NO;
    }
    
    result = IOHIDDeviceOpen(lidDevice, kIOHIDOptionsTypeNone);
    if (result != kIOReturnSuccess) {
        CFRelease(lidDevice);
        lidDevice = NULL;
        IOHIDManagerClose(hidManager, kIOHIDOptionsTypeNone);
        CFRelease(hidManager);
        hidManager = NULL;
        return NO;
    }
    
    return YES;
}

void LidSensorClose(void) {
    if (lidDevice) {
        IOHIDDeviceClose(lidDevice, kIOHIDOptionsTypeNone);
        CFRelease(lidDevice);
        lidDevice = NULL;
    }
    
    if (hidManager) {
        IOHIDManagerClose(hidManager, kIOHIDOptionsTypeNone);
        CFRelease(hidManager);
        hidManager = NULL;
    }
}

#define ANGLE_REPORT_ID   1
#define ANGLE_REPORT_SIZE 3

int LidSensorGetAngle(void) {
    if (!lidDevice) {
        return LID_ANGLE_ERROR;
    }
    
    uint8_t report[ANGLE_REPORT_SIZE] = {0};
    CFIndex reportLength = ANGLE_REPORT_SIZE;
    
    IOReturn result = IOHIDDeviceGetReport(
        lidDevice,
        kIOHIDReportTypeFeature,
        ANGLE_REPORT_ID,
        report,
        &reportLength
    );
    
    if (result != kIOReturnSuccess) {
        return LID_ANGLE_ERROR;
    }
    
    int angle = report[1] | (report[2] << 8);
    
    if (angle < 0 || angle > 180) {
        return LID_ANGLE_ERROR;
    }
    
    return angle;
}

BOOL LidSensorIsAvailable(void) {
    return (lidDevice != NULL);
}
