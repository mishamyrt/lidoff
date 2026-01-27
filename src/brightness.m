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

static CGDirectDisplayID brightnessTargetDisplay(void) {
    CGDirectDisplayID displays[16];
    CGDisplayCount count = 0;
    if (CGGetOnlineDisplayList(16, displays, &count) == kCGErrorSuccess) {
        for (CGDisplayCount i = 0; i < count; i++) {
            if (CGDisplayIsBuiltin(displays[i])) {
                return displays[i];
            }
        }
    }
    return CGMainDisplayID();
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
    int result = DSGetBrightness(brightnessTargetDisplay(), &brightness);
    return (result == 0) ? brightness : -1.0f;
}

BOOL BrightnessSet(float brightness) {
    loadDisplayServices();
    if (!displayServicesAvailable) return NO;
    
    if (brightness < 0.0f) brightness = 0.0f;
    if (brightness > 1.0f) brightness = 1.0f;
    return (DSSetBrightness(brightnessTargetDisplay(), brightness) == 0);
}

BOOL BrightnessIsDisplayAvailable(void) {
    loadDisplayServices();
    return displayServicesAvailable;
}
