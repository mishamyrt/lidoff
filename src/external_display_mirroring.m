//
//  external_display_mirroring.m
//  lidoff - external display control via dummy mirroring
//

#import "external_display.h"
#import <dlfcn.h>
#import <objc/message.h>
#import <objc/runtime.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

typedef struct {
    CGDirectDisplayID displayID;
    uint32_t *mirrorSetIDs;
    size_t mirrorSetCount;
} MirrorBackup;

static MirrorBackup *mirrorBackups = NULL;
static size_t mirrorBackupCount = 0;
static size_t mirrorBackupCapacity = 0;
static BOOL monitorPanelLoaded = NO;
static BOOL monitorPanelAvailable = NO;
static void *monitorPanelHandle = NULL;

static id objc_msgSend_id(id obj, SEL sel) {
    return ((id (*)(id, SEL))objc_msgSend)(obj, sel);
}

static id objc_msgSend_id_uint32(id obj, SEL sel, uint32_t value) {
    return ((id (*)(id, SEL, uint32_t))objc_msgSend)(obj, sel, value);
}

static id objc_msgSend_id_id(id obj, SEL sel, id value) {
    return ((id (*)(id, SEL, id))objc_msgSend)(obj, sel, value);
}

static void objc_msgSend_void(id obj, SEL sel) {
    ((void (*)(id, SEL))objc_msgSend)(obj, sel);
}

static void objc_msgSend_void_id(id obj, SEL sel, id value) {
    ((void (*)(id, SEL, id))objc_msgSend)(obj, sel, value);
}

static uint32_t objc_msgSend_u32(id obj, SEL sel) {
    return ((uint32_t (*)(id, SEL))objc_msgSend)(obj, sel);
}

static BOOL objc_msgSend_bool(id obj, SEL sel) {
    return ((BOOL (*)(id, SEL))objc_msgSend)(obj, sel);
}

static void clearMirrorBackups(void) {
    if (!mirrorBackups) {
        mirrorBackupCount = 0;
        mirrorBackupCapacity = 0;
        return;
    }
    
    for (size_t i = 0; i < mirrorBackupCount; i++) {
        free(mirrorBackups[i].mirrorSetIDs);
    }
    
    free(mirrorBackups);
    mirrorBackups = NULL;
    mirrorBackupCount = 0;
    mirrorBackupCapacity = 0;
}

void ExternalDisplayMirroringClearBackups(void) {
    clearMirrorBackups();
}

BOOL ExternalDisplayMirroringPrepare(size_t displayCount) {
    clearMirrorBackups();
    if (displayCount == 0) {
        return YES;
    }
    
    mirrorBackups = calloc(displayCount, sizeof(MirrorBackup));
    if (!mirrorBackups) {
        return NO;
    }
    
    mirrorBackupCapacity = displayCount;
    return YES;
}

void ExternalDisplayMirroringFinalize(void) {
    if (mirrorBackupCount == 0) {
        clearMirrorBackups();
    }
}

BOOL ExternalDisplayMirroringHasBackups(void) {
    return mirrorBackupCount > 0;
}

static void loadMonitorPanel(void) {
    if (monitorPanelLoaded) {
        return;
    }
    
    monitorPanelLoaded = YES;
    monitorPanelHandle = dlopen(
        "/System/Library/PrivateFrameworks/MonitorPanel.framework/MonitorPanel",
        RTLD_NOW
    );
    monitorPanelAvailable = (monitorPanelHandle != NULL);
}

static id mpDisplayManager(void) {
    loadMonitorPanel();
    if (!monitorPanelAvailable) {
        return nil;
    }
    
    Class managerClass = NSClassFromString(@"MPDisplayMgr");
    if (!managerClass) {
        return nil;
    }
    
    SEL selectors[] = {
        @selector(sharedManager),
        @selector(sharedDisplayManager),
        @selector(sharedInstance),
        @selector(shared)
    };
    
    for (size_t i = 0; i < sizeof(selectors) / sizeof(selectors[0]); i++) {
        if ([managerClass respondsToSelector:selectors[i]]) {
            return objc_msgSend_id(managerClass, selectors[i]);
        }
    }
    
    return nil;
}

static id mpDisplayWithID(id manager, CGDirectDisplayID displayID) {
    if (!manager) {
        return nil;
    }
    
    SEL selector = @selector(displayWithID:);
    if (![manager respondsToSelector:selector]) {
        return nil;
    }
    
    return objc_msgSend_id_uint32(manager, selector, (uint32_t)displayID);
}

static NSString *mpDisplayName(id display) {
    if (!display) {
        return nil;
    }
    
    SEL selector = @selector(displayName);
    if (![display respondsToSelector:selector]) {
        return nil;
    }
    
    return objc_msgSend_id(display, selector);
}

static BOOL mpDisplayGetID(id display, CGDirectDisplayID *displayIDOut) {
    if (!display || !displayIDOut) {
        return NO;
    }
    
    SEL selector = @selector(displayID);
    if (![display respondsToSelector:selector]) {
        return NO;
    }
    
    *displayIDOut = (CGDirectDisplayID)objc_msgSend_u32(display, selector);
    return YES;
}

static NSArray *mpMirrorSetForDisplay(id manager, id display) {
    if (!manager || !display) {
        return nil;
    }
    
    SEL selector = @selector(mirrorSetForDisplay:);
    if (![manager respondsToSelector:selector]) {
        return nil;
    }
    
    id result = objc_msgSend_id_id(manager, selector, display);
    if (!result) {
        return nil;
    }
    
    if ([result isKindOfClass:[NSSet class]]) {
        return [result allObjects];
    }
    if ([result isKindOfClass:[NSArray class]]) {
        return result;
    }
    
    return nil;
}

static NSArray *mpAllDisplays(id manager) {
    if (!manager) {
        return nil;
    }
    
    SEL selectors[] = {
        @selector(displays),
        @selector(allDisplays),
        @selector(activeDisplays),
        @selector(connectedDisplays)
    };
    
    for (size_t i = 0; i < sizeof(selectors) / sizeof(selectors[0]); i++) {
        if ([manager respondsToSelector:selectors[i]]) {
            id result = objc_msgSend_id(manager, selectors[i]);
            if (!result) {
                continue;
            }
            if ([result isKindOfClass:[NSSet class]]) {
                return [result allObjects];
            }
            if ([result isKindOfClass:[NSArray class]]) {
                return result;
            }
        }
    }
    
    return nil;
}

static BOOL mpIsDummyDisplay(id display) {
    NSString *name = mpDisplayName(display);
    if (name.length > 0) {
        NSString *lower = name.lowercaseString;
        if ([lower containsString:@"dummy"] || [lower containsString:@"virtual"]) {
            return YES;
        }
    }
    
    SEL isDummySelector = @selector(isDummy);
    if ([display respondsToSelector:isDummySelector]) {
        return objc_msgSend_bool(display, isDummySelector);
    }
    
    return NO;
}

static id mpFindDummyDisplayInSet(NSArray *mirrorSet, CGDirectDisplayID excludeID) {
    if (!mirrorSet) {
        return nil;
    }
    
    for (id display in mirrorSet) {
        CGDirectDisplayID displayID = 0;
        if (mpDisplayGetID(display, &displayID) && displayID == excludeID) {
            continue;
        }
        if (mpIsDummyDisplay(display)) {
            return display;
        }
    }
    
    return nil;
}

static id mpFindDummyDisplay(id manager, CGDirectDisplayID excludeID) {
    NSArray *displays = mpAllDisplays(manager);
    for (id display in displays) {
        CGDirectDisplayID displayID = 0;
        if (mpDisplayGetID(display, &displayID) && displayID == excludeID) {
            continue;
        }
        if (mpIsDummyDisplay(display)) {
            return display;
        }
    }
    
    return nil;
}

static void mpNotifyWillReconfigure(id manager) {
    SEL selector = @selector(notifyWillReconfigure);
    if ([manager respondsToSelector:selector]) {
        objc_msgSend_void(manager, selector);
    }
}

static void mpNotifyReconfigure(id manager) {
    SEL selector = @selector(notifyReconfigure);
    if ([manager respondsToSelector:selector]) {
        objc_msgSend_void(manager, selector);
    }
}

static void mpUnlockAccess(id manager) {
    SEL selector = @selector(unlockAccess);
    if ([manager respondsToSelector:selector]) {
        objc_msgSend_void(manager, selector);
    }
}

static BOOL mpCreateMirrorSet(id manager, NSArray *mirrorSet) {
    if (!manager || mirrorSet.count == 0) {
        return NO;
    }
    
    SEL selector = @selector(createMirrorSet:);
    if (![manager respondsToSelector:selector]) {
        return NO;
    }
    
    objc_msgSend_void_id(manager, selector, mirrorSet);
    return YES;
}

static BOOL mpStopMirroringForDisplay(id manager, id display) {
    if (!manager || !display) {
        return NO;
    }
    
    SEL selector = @selector(stopMirroringForDisplay:);
    if (![manager respondsToSelector:selector]) {
        return NO;
    }
    
    objc_msgSend_void_id(manager, selector, display);
    return YES;
}

static void resetMirrorBackup(MirrorBackup *backup) {
    if (!backup) {
        return;
    }
    free(backup->mirrorSetIDs);
    backup->mirrorSetIDs = NULL;
    backup->mirrorSetCount = 0;
    backup->displayID = 0;
}

static void buildMirrorBackupFromSet(NSArray *mirrorSet, MirrorBackup *backup) {
    if (!backup) {
        return;
    }
    
    backup->mirrorSetIDs = NULL;
    backup->mirrorSetCount = 0;
    if (!mirrorSet || mirrorSet.count == 0) {
        return;
    }
    
    uint32_t *ids = calloc((size_t)mirrorSet.count, sizeof(uint32_t));
    if (!ids) {
        return;
    }
    
    size_t count = 0;
    for (id display in mirrorSet) {
        CGDirectDisplayID displayID = 0;
        if (mpDisplayGetID(display, &displayID)) {
            ids[count++] = (uint32_t)displayID;
        }
    }
    
    if (count == 0) {
        free(ids);
        return;
    }
    
    backup->mirrorSetIDs = ids;
    backup->mirrorSetCount = count;
}

static BOOL monitorPanelBlackoutDisplay(CGDirectDisplayID displayID, MirrorBackup *backupOut) {
    id manager = mpDisplayManager();
    if (!manager) {
        return NO;
    }
    
    id display = mpDisplayWithID(manager, displayID);
    if (!display) {
        return NO;
    }
    
    NSArray *mirrorSet = mpMirrorSetForDisplay(manager, display);
    id dummyDisplay = mpFindDummyDisplayInSet(mirrorSet, displayID);
    if (!dummyDisplay) {
        dummyDisplay = mpFindDummyDisplay(manager, displayID);
    }
    if (!dummyDisplay) {
        return NO;
    }
    
    NSMutableArray *newMirrorSet = [NSMutableArray array];
    if (mirrorSet.count > 0) {
        [newMirrorSet addObjectsFromArray:mirrorSet];
    }
    if (![newMirrorSet containsObject:display]) {
        [newMirrorSet addObject:display];
    }
    if (![newMirrorSet containsObject:dummyDisplay]) {
        [newMirrorSet addObject:dummyDisplay];
    }
    if (newMirrorSet.count < 2) {
        return NO;
    }
    
    MirrorBackup backup = {0};
    backup.displayID = displayID;
    buildMirrorBackupFromSet(mirrorSet, &backup);
    
    mpNotifyWillReconfigure(manager);
    BOOL ok = mpCreateMirrorSet(manager, newMirrorSet);
    mpNotifyReconfigure(manager);
    mpUnlockAccess(manager);
    
    if (!ok) {
        resetMirrorBackup(&backup);
        return NO;
    }
    
    if (backupOut) {
        *backupOut = backup;
    } else {
        resetMirrorBackup(&backup);
    }
    
    return YES;
}

static BOOL monitorPanelRestoreDisplay(const MirrorBackup *backup) {
    if (!backup) {
        return NO;
    }
    
    id manager = mpDisplayManager();
    if (!manager) {
        return NO;
    }
    
    id display = mpDisplayWithID(manager, backup->displayID);
    if (!display) {
        return NO;
    }
    
    BOOL didRestore = NO;
    if (backup->mirrorSetCount > 0 && backup->mirrorSetIDs) {
        NSMutableArray *restoreSet = [NSMutableArray arrayWithCapacity:backup->mirrorSetCount];
        for (size_t i = 0; i < backup->mirrorSetCount; i++) {
            id mpDisplay = mpDisplayWithID(manager, backup->mirrorSetIDs[i]);
            if (mpDisplay) {
                [restoreSet addObject:mpDisplay];
            }
        }
        if (restoreSet.count > 0) {
            mpNotifyWillReconfigure(manager);
            didRestore = mpCreateMirrorSet(manager, restoreSet);
            mpNotifyReconfigure(manager);
            mpUnlockAccess(manager);
        }
    }
    
    if (!didRestore) {
        didRestore = mpStopMirroringForDisplay(manager, display);
    }
    
    return didRestore;
}

BOOL ExternalDisplayMirroringDisableDisplay(CGDirectDisplayID displayID) {
    MirrorBackup mirrorBackup = {0};
    if (!monitorPanelBlackoutDisplay(displayID, &mirrorBackup)) {
        return NO;
    }
    
    if (!mirrorBackups || mirrorBackupCount >= mirrorBackupCapacity) {
        monitorPanelRestoreDisplay(&mirrorBackup);
        resetMirrorBackup(&mirrorBackup);
        return NO;
    }
    
    mirrorBackups[mirrorBackupCount++] = mirrorBackup;
    return YES;
}

void ExternalDisplayMirroringRestoreAll(void) {
    for (size_t i = 0; i < mirrorBackupCount; i++) {
        monitorPanelRestoreDisplay(&mirrorBackups[i]);
    }
    clearMirrorBackups();
}
