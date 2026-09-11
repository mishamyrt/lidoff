#include "skylight.h"

#include <CoreGraphics/CGDisplayConfiguration.h>
#include <dlfcn.h>
#include <stdbool.h>

typedef CGError (*SLSConfigureDisplayEnabledFunc)(CGDisplayConfigRef,
                                                  CGDirectDisplayID,
                                                  Boolean);

static bool skylight_loaded = false;
static bool skylight_available = false;
static void *skylight_handle = NULL;
static SLSConfigureDisplayEnabledFunc SLSConfigureDisplayEnabledPtr = NULL;
static CGDisplayConfigRef skylight_config = NULL;
static bool skylight_config_dirty = false;

uint8_t SkylightFinalize(void) {
    uint8_t ok = 1;

    if (skylight_config != NULL) {
        if (skylight_config_dirty) {
            CGError err =
                CGCompleteDisplayConfiguration(skylight_config, kCGConfigureForSession);
            if (err != kCGErrorSuccess) {
                CGCancelDisplayConfiguration(skylight_config);
                ok = 0;
            }
        } else {
            CGCancelDisplayConfiguration(skylight_config);
        }

        skylight_config = NULL;
        skylight_config_dirty = false;
    }

    return ok;
}

static void loadSkylight(void) {
    if (skylight_loaded) {
        return;
    }

    skylight_loaded = true;
    skylight_handle =
        dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", RTLD_NOW);
    if (skylight_handle == NULL) {
        return;
    }

    SLSConfigureDisplayEnabledPtr =
        (SLSConfigureDisplayEnabledFunc)dlsym(skylight_handle, "SLSConfigureDisplayEnabled");
    skylight_available = (SLSConfigureDisplayEnabledPtr != NULL);
}

static bool skylightSetDisplayEnabled(CGDirectDisplayID display_id, bool enabled) {
    loadSkylight();
    if (!skylight_available) {
        return false;
    }

    if (skylight_config == NULL) {
        CGError err = CGBeginDisplayConfiguration(&skylight_config);
        if (err != kCGErrorSuccess || skylight_config == NULL) {
            return false;
        }
    }

    CGError err = SLSConfigureDisplayEnabledPtr(skylight_config, display_id, enabled ? 1 : 0);
    if (err != kCGErrorSuccess) {
        return false;
    }

    skylight_config_dirty = true;
    return true;
}

uint8_t SkylightDisableDisplay(CGDirectDisplayID display_id) {
    return skylightSetDisplayEnabled(display_id, false) ? 1 : 0;
}

size_t SkylightRestoreFromState(const CGDirectDisplayID *display_ids, size_t count) {
    if (display_ids == NULL && count > 0) {
        return 0;
    }

    size_t restored = 0;
    for (size_t i = 0; i < count; i++) {
        if (skylightSetDisplayEnabled(display_ids[i], true)) {
            restored++;
        }
    }

    if (!SkylightFinalize()) {
        restored = 0;
    }

    return restored;
}
