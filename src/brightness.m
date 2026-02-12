//
//  brightness.m
//  lidoff - display brightness control
//

#import "brightness.h"
#import <dlfcn.h>
#import <CoreGraphics/CoreGraphics.h>

typedef int (*DSGetBrightnessFunc)(CGDirectDisplayID, float *);
typedef int (*DSSetBrightnessFunc)(CGDirectDisplayID, float);

static DSGetBrightnessFunc DSGetBrightness = NULL;
static DSSetBrightnessFunc DSSetBrightness = NULL;
static BOOL displayServicesLoaded = NO;
static BOOL displayServicesAvailable = NO;
static CGDirectDisplayID cachedBuiltinDisplayID = kCGNullDirectDisplay;

static CGDirectDisplayID brightnessTargetDisplay(void) {
    CGDirectDisplayID displays[16];
    CGDisplayCount count = 0;
    if (CGGetOnlineDisplayList(16, displays, &count) == kCGErrorSuccess) {
        for (CGDisplayCount i = 0; i < count; i++) {
            if (CGDisplayIsBuiltin(displays[i])) {
                cachedBuiltinDisplayID = displays[i];
                return cachedBuiltinDisplayID;
            }
        }
    }
    return cachedBuiltinDisplayID;
}

static void loadDisplayServices(void) {
    if (displayServicesLoaded) return;
    displayServicesLoaded = YES;
    
    void *handle = dlopen("/System/Library/PrivateFrameworks/DisplayServices.framework/DisplayServices", RTLD_NOW);
    if (!handle) return;
    
    DSGetBrightness = (DSGetBrightnessFunc)dlsym(handle, "DisplayServicesGetBrightness");
    DSSetBrightness = (DSSetBrightnessFunc)dlsym(handle, "DisplayServicesSetBrightness");
    
    if (DSGetBrightness && DSSetBrightness) {
        displayServicesAvailable = YES;
    }
}

float BrightnessGet(void) {
    loadDisplayServices();
    if (!displayServicesAvailable) return -1.0f;

    float brightness = 0.0f;
    CGDirectDisplayID targetDisplay = brightnessTargetDisplay();
    if (targetDisplay != kCGNullDirectDisplay &&
        DSGetBrightness(targetDisplay, &brightness) == 0) {
        return brightness;
    }

    cachedBuiltinDisplayID = kCGNullDirectDisplay;
    targetDisplay = brightnessTargetDisplay();
    if (targetDisplay != kCGNullDirectDisplay &&
        DSGetBrightness(targetDisplay, &brightness) == 0) {
        return brightness;
    }

    return -1.0f;
}

BOOL BrightnessSet(float brightness) {
    loadDisplayServices();
    if (!displayServicesAvailable) return NO;

    if (brightness < 0.0f) brightness = 0.0f;
    if (brightness > 1.0f) brightness = 1.0f;

    CGDirectDisplayID targetDisplay = brightnessTargetDisplay();
    if (targetDisplay != kCGNullDirectDisplay &&
        DSSetBrightness(targetDisplay, brightness) == 0) {
        return YES;
    }

    cachedBuiltinDisplayID = kCGNullDirectDisplay;
    targetDisplay = brightnessTargetDisplay();
    if (targetDisplay != kCGNullDirectDisplay &&
        DSSetBrightness(targetDisplay, brightness) == 0) {
        return YES;
    }

    return NO;
}

BOOL BrightnessIsDisplayAvailable(void) {
    loadDisplayServices();
    return displayServicesAvailable;
}
