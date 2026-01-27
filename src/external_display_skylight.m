//
//  external_display_skylight.m
//  lidoff - external display control via Skylight
//

#import "external_display.h"
#import <CoreGraphics/CGDisplayConfiguration.h>
#import <dlfcn.h>
#include <stdlib.h>

typedef CGError (*SLSConfigureDisplayEnabledFunc)(CGDisplayConfigRef, CGDirectDisplayID, Boolean);

static CGDirectDisplayID *skylightBackups = NULL;
static size_t skylightBackupCount = 0;
static size_t skylightBackupCapacity = 0;
static BOOL skylightLoaded = NO;
static BOOL skylightAvailable = NO;
static void *skylightHandle = NULL;
static SLSConfigureDisplayEnabledFunc SLSConfigureDisplayEnabledPtr = NULL;

static void clearSkylightBackups(void) {
    if (!skylightBackups) {
        skylightBackupCount = 0;
        skylightBackupCapacity = 0;
        return;
    }
    
    free(skylightBackups);
    skylightBackups = NULL;
    skylightBackupCount = 0;
    skylightBackupCapacity = 0;
}

void ExternalDisplaySkylightClearBackups(void) {
    clearSkylightBackups();
}

BOOL ExternalDisplaySkylightPrepare(size_t displayCount) {
    clearSkylightBackups();
    if (displayCount == 0) {
        return YES;
    }
    
    skylightBackups = calloc(displayCount, sizeof(CGDirectDisplayID));
    if (!skylightBackups) {
        return NO;
    }
    
    skylightBackupCapacity = displayCount;
    return YES;
}

void ExternalDisplaySkylightFinalize(void) {
    if (skylightBackupCount == 0) {
        clearSkylightBackups();
    }
}

BOOL ExternalDisplaySkylightHasBackups(void) {
    return skylightBackupCount > 0;
}

static void loadSkyLight(void) {
    if (skylightLoaded) {
        return;
    }
    
    skylightLoaded = YES;
    skylightHandle = dlopen(
        "/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight",
        RTLD_NOW
    );
    if (!skylightHandle) {
        return;
    }
    
    SLSConfigureDisplayEnabledPtr = (SLSConfigureDisplayEnabledFunc)dlsym(
        skylightHandle,
        "SLSConfigureDisplayEnabled"
    );
    skylightAvailable = (SLSConfigureDisplayEnabledPtr != NULL);
}

static BOOL skylightSetDisplayEnabled(CGDirectDisplayID displayID, BOOL enabled) {
    loadSkyLight();
    if (!skylightAvailable) {
        return NO;
    }
    
    CGDisplayConfigRef config = NULL;
    CGError err = CGBeginDisplayConfiguration(&config);
    if (err != kCGErrorSuccess || !config) {
        return NO;
    }
    
    err = SLSConfigureDisplayEnabledPtr(config, displayID, enabled ? 1 : 0);
    if (err != kCGErrorSuccess) {
        CGCancelDisplayConfiguration(config);
        return NO;
    }
    
    err = CGCompleteDisplayConfiguration(config, kCGConfigurePermanently);
    if (err != kCGErrorSuccess) {
        CGCancelDisplayConfiguration(config);
        return NO;
    }
    
    return YES;
}

BOOL ExternalDisplaySkylightDisableDisplay(CGDirectDisplayID displayID) {
    if (!skylightSetDisplayEnabled(displayID, NO)) {
        return NO;
    }
    
    if (!skylightBackups || skylightBackupCount >= skylightBackupCapacity) {
        skylightSetDisplayEnabled(displayID, YES);
        return NO;
    }
    
    skylightBackups[skylightBackupCount++] = displayID;
    return YES;
}

void ExternalDisplaySkylightRestoreAll(void) {
    for (size_t i = 0; i < skylightBackupCount; i++) {
        skylightSetDisplayEnabled(skylightBackups[i], YES);
    }
    clearSkylightBackups();
}
