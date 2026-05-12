#include "skylight.h"

#include <CoreGraphics/CGDisplayConfiguration.h>
#include <dlfcn.h>
#include <stdbool.h>
#include <stdlib.h>

typedef CGError (*SLSConfigureDisplayEnabledFunc)(CGDisplayConfigRef,
                                                  CGDirectDisplayID,
                                                  Boolean);

static CGDirectDisplayID *skylight_backups = NULL;
static size_t skylight_backup_count = 0;
static size_t skylight_backup_capacity = 0;
static bool skylight_loaded = false;
static bool skylight_available = false;
static void *skylight_handle = NULL;
static SLSConfigureDisplayEnabledFunc SLSConfigureDisplayEnabledPtr = NULL;
static CGDisplayConfigRef skylight_config = NULL;
static bool skylight_config_dirty = false;

static void clearSkylightBackups(void) {
    if (skylight_backups == NULL) {
        skylight_backup_count = 0;
        skylight_backup_capacity = 0;
        return;
    }

    free(skylight_backups);
    skylight_backups = NULL;
    skylight_backup_count = 0;
    skylight_backup_capacity = 0;
}

void SkylightClearBackups(void) {
    clearSkylightBackups();
}

uint8_t SkylightPrepare(size_t display_count) {
    clearSkylightBackups();
    if (display_count == 0) {
        return 1;
    }

    skylight_backups = calloc(display_count, sizeof(*skylight_backups));
    if (skylight_backups == NULL) {
        return 0;
    }

    skylight_backup_capacity = display_count;
    return 1;
}

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

    if (skylight_backup_count == 0) {
        clearSkylightBackups();
    }

    return ok;
}

size_t SkylightBackupCount(void) {
    return skylight_backup_count;
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

uint8_t SkylightCaptureDisplay(CGDirectDisplayID display_id) {
    if (display_id == kCGNullDirectDisplay || skylight_backups == NULL ||
        skylight_backup_count >= skylight_backup_capacity) {
        return 0;
    }

    skylight_backups[skylight_backup_count++] = display_id;
    return 1;
}

uint8_t SkylightDisableDisplay(CGDirectDisplayID display_id) {
    if (!skylightSetDisplayEnabled(display_id, false)) {
        return 0;
    }

    if (skylight_backups == NULL || skylight_backup_count >= skylight_backup_capacity) {
        skylightSetDisplayEnabled(display_id, true);
        return 0;
    }

    skylight_backups[skylight_backup_count++] = display_id;
    return 1;
}

size_t SkylightRestoreAll(void) {
    if (skylight_backups == NULL || skylight_backup_count == 0) {
        return 0;
    }

    size_t restored = 0;
    size_t remaining = 0;
    for (size_t i = 0; i < skylight_backup_count; i++) {
        CGDirectDisplayID display_id = skylight_backups[i];
        if (skylightSetDisplayEnabled(display_id, true)) {
            restored++;
            continue;
        }

        skylight_backups[remaining++] = display_id;
    }

    if (!SkylightFinalize()) {
        restored = 0;
        remaining = skylight_backup_count;
    }

    skylight_backup_count = remaining;
    if (skylight_backup_count == 0) {
        clearSkylightBackups();
    }

    return restored;
}

size_t SkylightCopyState(CGDirectDisplayID *display_ids, size_t capacity) {
    if (display_ids == NULL || capacity == 0) {
        return 0;
    }

    size_t copied = skylight_backup_count < capacity ? skylight_backup_count : capacity;
    for (size_t i = 0; i < copied; i++) {
        display_ids[i] = skylight_backups[i];
    }

    return copied;
}

size_t SkylightRestoreFromState(const CGDirectDisplayID *display_ids, size_t count) {
    if (display_ids == NULL && count > 0) {
        clearSkylightBackups();
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

    clearSkylightBackups();
    return restored;
}
