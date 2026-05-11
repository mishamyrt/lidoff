#include "brightness.h"

#include <CoreGraphics/CoreGraphics.h>
#include <dlfcn.h>
#include <stdbool.h>

typedef int (*DSGetBrightnessFunc)(CGDirectDisplayID, float *);
typedef int (*DSSetBrightnessFunc)(CGDirectDisplayID, float);

static DSGetBrightnessFunc DSGetBrightness = NULL;
static DSSetBrightnessFunc DSSetBrightness = NULL;
static bool display_services_loaded = false;
static bool display_services_available = false;
static CGDirectDisplayID cached_builtin_display_id = kCGNullDirectDisplay;

static CGDirectDisplayID brightnessTargetDisplay(void) {
    CGDirectDisplayID displays[16];
    CGDisplayCount count = 0;
    if (CGGetOnlineDisplayList(16, displays, &count) == kCGErrorSuccess) {
        for (CGDisplayCount i = 0; i < count; i++) {
            if (CGDisplayIsBuiltin(displays[i])) {
                cached_builtin_display_id = displays[i];
                return cached_builtin_display_id;
            }
        }
    }

    return cached_builtin_display_id;
}

static CGDirectDisplayID resolveBuiltinDisplay(void) {
    CGDirectDisplayID display = brightnessTargetDisplay();
    if (display != kCGNullDirectDisplay) {
        return display;
    }

    cached_builtin_display_id = kCGNullDirectDisplay;
    return brightnessTargetDisplay();
}

static void loadDisplayServices(void) {
    if (display_services_loaded) {
        return;
    }

    display_services_loaded = true;
    void *handle = dlopen(
        "/System/Library/PrivateFrameworks/"
        "DisplayServices.framework/DisplayServices",
        RTLD_NOW);
    if (handle == NULL) {
        return;
    }

    DSGetBrightness = (DSGetBrightnessFunc)dlsym(handle, "DisplayServicesGetBrightness");
    DSSetBrightness = (DSSetBrightnessFunc)dlsym(handle, "DisplayServicesSetBrightness");
    display_services_available = (DSGetBrightness != NULL && DSSetBrightness != NULL);
}

float BrightnessGet(void) {
    loadDisplayServices();
    if (!display_services_available) {
        return -1.0f;
    }

    float brightness = 0.0f;
    CGDirectDisplayID display = resolveBuiltinDisplay();
    if (display != kCGNullDirectDisplay && DSGetBrightness(display, &brightness) == 0) {
        return brightness;
    }

    return -1.0f;
}

uint8_t BrightnessSet(float brightness) {
    loadDisplayServices();
    if (!display_services_available) {
        return 0;
    }

    if (brightness < 0.0f) {
        brightness = 0.0f;
    }
    if (brightness > 1.0f) {
        brightness = 1.0f;
    }

    CGDirectDisplayID display = resolveBuiltinDisplay();
    if (display != kCGNullDirectDisplay && DSSetBrightness(display, brightness) == 0) {
        return 1;
    }

    return 0;
}
