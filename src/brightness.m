//
//  brightness.m
//  lidoff - display brightness control
//
//  Supports DisplayServices (Apple Silicon) and IOKit (Intel Mac)
//

#import "brightness.h"
#import <dlfcn.h>
#import <CoreGraphics/CoreGraphics.h>
#import <IOKit/IOKitLib.h>
#import <IOKit/graphics/IOGraphicsLib.h>

typedef int (*DSGetBrightnessFunc)(CGDirectDisplayID, float *);
typedef int (*DSSetBrightnessFunc)(CGDirectDisplayID, float);

static DSGetBrightnessFunc DSGetBrightness = NULL;
static DSSetBrightnessFunc DSSetBrightness = NULL;
static BOOL displayServicesLoaded = NO;
static BOOL displayServicesAvailable = NO;

static const CFStringRef kDisplayBrightnessKey = CFSTR(kIODisplayBrightnessKey);

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

static io_service_t getDisplayService(void) {
    io_iterator_t iterator;
    io_service_t service = 0;
    
    kern_return_t result = IOServiceGetMatchingServices(
        kIOMainPortDefault,
        IOServiceMatching("IODisplayConnect"),
        &iterator
    );
    
    if (result != KERN_SUCCESS) {
        return 0;
    }
    
    service = IOIteratorNext(iterator);
    IOObjectRelease(iterator);
    
    return service;
}

static float DSBrightnessGet(void) {
    float brightness = 0.0f;
    int result = DSGetBrightness(CGMainDisplayID(), &brightness);
    return (result == 0) ? brightness : -1.0f;
}

static BOOL DSBrightnessSet(float brightness) {
    if (brightness < 0.0f) brightness = 0.0f;
    if (brightness > 1.0f) brightness = 1.0f;
    return (DSSetBrightness(CGMainDisplayID(), brightness) == 0);
}

static float IOKitBrightnessGet(void) {
    io_service_t service = getDisplayService();
    if (!service) return -1.0f;
    
    float brightness = 0.0f;
    IOReturn result = IODisplayGetFloatParameter(service, kNilOptions, kDisplayBrightnessKey, &brightness);
    IOObjectRelease(service);
    
    return (result == kIOReturnSuccess) ? brightness : -1.0f;
}

static BOOL IOKitBrightnessSet(float brightness) {
    if (brightness < 0.0f) brightness = 0.0f;
    if (brightness > 1.0f) brightness = 1.0f;
    
    io_service_t service = getDisplayService();
    if (!service) return NO;
    
    IOReturn result = IODisplaySetFloatParameter(service, kNilOptions, kDisplayBrightnessKey, brightness);
    IOObjectRelease(service);
    
    return (result == kIOReturnSuccess);
}

float BrightnessGet(void) {
    loadDisplayServices();
    return displayServicesAvailable ? DSBrightnessGet() : IOKitBrightnessGet();
}

BOOL BrightnessSet(float brightness) {
    loadDisplayServices();
    return displayServicesAvailable ? DSBrightnessSet(brightness) : IOKitBrightnessSet(brightness);
}

BOOL BrightnessIsDisplayAvailable(void) {
    loadDisplayServices();
    if (displayServicesAvailable) return YES;
    
    io_service_t service = getDisplayService();
    if (service) {
        IOObjectRelease(service);
        return YES;
    }
    return NO;
}
