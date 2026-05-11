#include "lid_sensor.h"

#include <CoreFoundation/CoreFoundation.h>
#include <IOKit/hid/IOHIDDevice.h>
#include <IOKit/hid/IOHIDManager.h>

#include <stdint.h>
#include <stdlib.h>

static IOHIDManagerRef hidManager = NULL;
static IOHIDDeviceRef lidDevice = NULL;
static CFIndex lidReportID = 1;

#define LID_SENSOR_VID 0x05AC
#define LID_SENSOR_PID 0x8104
#define LID_SENSOR_USAGE_PAGE 0x0020
#define LID_SENSOR_USAGE 0x008A

#define LID_ANGLE_ERROR -1

#define ANGLE_PRIMARY_REPORT_ID 1
#define ANGLE_FALLBACK_REPORT_ID 0
#define ANGLE_REPORT_SIZE 8

static bool readAngleReport(IOHIDDeviceRef device,
                            CFIndex reportID,
                            uint8_t report[ANGLE_REPORT_SIZE]) {
    CFIndex reportLength = ANGLE_REPORT_SIZE;

    IOReturn result =
        IOHIDDeviceGetReport(device, kIOHIDReportTypeFeature, reportID, report, &reportLength);

    return result == kIOReturnSuccess && reportLength >= 3;
}

static bool detectReportID(IOHIDDeviceRef device, CFIndex *reportID) {
    const CFIndex reportIDs[] = {
        ANGLE_PRIMARY_REPORT_ID,
        ANGLE_FALLBACK_REPORT_ID,
    };

    uint8_t report[ANGLE_REPORT_SIZE] = {0};

    for (size_t i = 0; i < sizeof(reportIDs) / sizeof(reportIDs[0]); i++) {
        if (readAngleReport(device, reportIDs[i], report)) {
            *reportID = reportIDs[i];
            return true;
        }
    }

    return false;
}

static bool getIntDeviceProperty(IOHIDDeviceRef device, CFStringRef key, int *value) {
    CFTypeRef ref = IOHIDDeviceGetProperty(device, key);
    if (!ref || CFGetTypeID(ref) != CFNumberGetTypeID()) {
        return false;
    }

    return CFNumberGetValue((CFNumberRef)ref, kCFNumberIntType, value);
}

static bool isLidAngleSensor(IOHIDDeviceRef device) {
    int vid = 0;
    if (!getIntDeviceProperty(device, CFSTR(kIOHIDVendorIDKey), &vid)) {
        return false;
    }
    if (vid != LID_SENSOR_VID) {
        return false;
    }

    int pid = 0;
    if (!getIntDeviceProperty(device, CFSTR(kIOHIDProductIDKey), &pid)) {
        return false;
    }
    if (pid != LID_SENSOR_PID) {
        return false;
    }

    int usagePage = 0;
    if (!getIntDeviceProperty(device, CFSTR(kIOHIDPrimaryUsagePageKey), &usagePage)) {
        return false;
    }
    if (usagePage != LID_SENSOR_USAGE_PAGE) {
        return false;
    }

    int usage = 0;
    if (!getIntDeviceProperty(device, CFSTR(kIOHIDPrimaryUsageKey), &usage)) {
        return false;
    }
    if (usage != LID_SENSOR_USAGE) {
        return false;
    }

    return true;
}

static IOHIDDeviceRef findLidSensorDevice(IOHIDManagerRef manager) {
    CFSetRef deviceSet = IOHIDManagerCopyDevices(manager);
    if (!deviceSet) {
        return NULL;
    }

    CFIndex count = CFSetGetCount(deviceSet);
    if (count == 0) {
        CFRelease(deviceSet);
        return NULL;
    }

    IOHIDDeviceRef *devices = malloc(sizeof(IOHIDDeviceRef) * (size_t)count);
    if (!devices) {
        CFRelease(deviceSet);
        return NULL;
    }

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

static CFDictionaryRef createHIDMatchingDictionary(void) {
    int vid = LID_SENSOR_VID;
    int pid = LID_SENSOR_PID;

    CFNumberRef vidRef = CFNumberCreate(kCFAllocatorDefault, kCFNumberIntType, &vid);
    if (!vidRef) {
        return NULL;
    }

    CFNumberRef pidRef = CFNumberCreate(kCFAllocatorDefault, kCFNumberIntType, &pid);
    if (!pidRef) {
        CFRelease(vidRef);
        return NULL;
    }

    const void *keys[] = {
        CFSTR(kIOHIDVendorIDKey),
        CFSTR(kIOHIDProductIDKey),
    };

    const void *values[] = {
        vidRef,
        pidRef,
    };

    CFDictionaryRef matching =
        CFDictionaryCreate(kCFAllocatorDefault, keys, values, 2,
                           &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);

    CFRelease(vidRef);
    CFRelease(pidRef);

    return matching;
}

bool LidSensorInit(void) {
    if (hidManager && lidDevice) {
        return true;
    }

    if (hidManager || lidDevice) {
        LidSensorClose();
    }

    hidManager = IOHIDManagerCreate(kCFAllocatorDefault, kIOHIDOptionsTypeNone);
    if (!hidManager) {
        return false;
    }

    CFDictionaryRef matching = createHIDMatchingDictionary();
    if (!matching) {
        CFRelease(hidManager);
        hidManager = NULL;
        return false;
    }

    IOHIDManagerSetDeviceMatching(hidManager, matching);
    CFRelease(matching);

    IOReturn result = IOHIDManagerOpen(hidManager, kIOHIDOptionsTypeNone);
    if (result != kIOReturnSuccess) {
        CFRelease(hidManager);
        hidManager = NULL;
        return false;
    }

    lidDevice = findLidSensorDevice(hidManager);
    if (!lidDevice) {
        IOHIDManagerClose(hidManager, kIOHIDOptionsTypeNone);
        CFRelease(hidManager);
        hidManager = NULL;
        return false;
    }

    result = IOHIDDeviceOpen(lidDevice, kIOHIDOptionsTypeNone);
    if (result != kIOReturnSuccess) {
        CFRelease(lidDevice);
        lidDevice = NULL;

        IOHIDManagerClose(hidManager, kIOHIDOptionsTypeNone);
        CFRelease(hidManager);
        hidManager = NULL;

        return false;
    }

    CFIndex reportID = ANGLE_PRIMARY_REPORT_ID;
    if (!detectReportID(lidDevice, &reportID)) {
        LidSensorClose();
        return false;
    }

    lidReportID = reportID;
    return true;
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

    lidReportID = ANGLE_PRIMARY_REPORT_ID;
}

int LidSensorGetAngle(void) {
    if (!lidDevice) {
        return LID_ANGLE_ERROR;
    }

    uint8_t report[ANGLE_REPORT_SIZE] = {0};

    if (!readAngleReport(lidDevice, lidReportID, report)) {
        return LID_ANGLE_ERROR;
    }

    int angle = report[1] | (report[2] << 8);

    if (angle < 0 || angle > 180) {
        return LID_ANGLE_ERROR;
    }

    return angle;
}
