#import "external_display.h"
#import "external_display_backend.h"
#import "logging.h"

static BOOL externalDisplaysDisabled = NO;
static BOOL backendsInitialized = NO;

static const ExternalDisplayBackend *kBackends[] = {
    NULL,
    NULL,
    NULL
};

static const ExternalDisplayBackend *backendAtIndex(size_t index) {
    if (index >= sizeof(kBackends) / sizeof(kBackends[0])) {
        return NULL;
    }
    return kBackends[index];
}

static size_t backendCount(void) {
    return sizeof(kBackends) / sizeof(kBackends[0]);
}

static void ensureBackendsInitialized(void) {
    if (backendsInitialized) {
        return;
    }
    // Full blackout path first; gamma remains last-resort fallback.
    kBackends[0] = ExternalDisplayBackendMirroring();
    kBackends[1] = ExternalDisplayBackendSkylight();
    kBackends[2] = ExternalDisplayBackendGamma();
    LogInfo(@"external display backends: mirroring -> skylight -> gamma");
    backendsInitialized = YES;
}

static NSString *backendKey(const ExternalDisplayBackend *backend) {
    if (!backend || !backend->name) {
        return nil;
    }
    return [NSString stringWithUTF8String:backend->name];
}

static void clearBackupsForAllBackends(void) {
    for (size_t i = 0; i < backendCount(); i++) {
        const ExternalDisplayBackend *backend = backendAtIndex(i);
        if (backend && backend->clearBackups) {
            backend->clearBackups();
        }
    }
}

static BOOL prepareAllBackends(CGDisplayCount displayCount) {
    for (size_t i = 0; i < backendCount(); i++) {
        const ExternalDisplayBackend *backend = backendAtIndex(i);
        if (!backend || !backend->prepare) {
            continue;
        }
        if (!backend->prepare(displayCount)) {
            LogError(@"external display backend %s failed to prepare", backend->name);
            return NO;
        }
    }
    return YES;
}

static void finalizeAllBackends(void) {
    for (size_t i = 0; i < backendCount(); i++) {
        const ExternalDisplayBackend *backend = backendAtIndex(i);
        if (backend && backend->finalize) {
            backend->finalize();
        }
    }
}

ExternalDisplayDisableResult ExternalDisplaysDisable(void) {
    ensureBackendsInitialized();
    ExternalDisplayDisableResult result = {
        .ok = YES,
        .alreadyDisabled = externalDisplaysDisabled,
        .totalExternal = 0,
        .disabled = 0,
        .failed = 0
    };
    
    if (externalDisplaysDisabled) {
        return result;
    }
    
    CGDirectDisplayID displays[32];
    CGDisplayCount displayCount = 0;
    CGError err = CGGetOnlineDisplayList(32, displays, &displayCount);
    if (err != kCGErrorSuccess) {
        LogError(@"failed to enumerate displays (error %d)", err);
        result.ok = NO;
        return result;
    }
    
    if (!prepareAllBackends(displayCount)) {
        clearBackupsForAllBackends();
        result.ok = NO;
        return result;
    }
    
    for (CGDisplayCount i = 0; i < displayCount; i++) {
        CGDirectDisplayID displayID = displays[i];
        if (CGDisplayIsBuiltin(displayID)) {
            continue;
        }
        
        result.totalExternal++;
        BOOL disabled = NO;
        
        for (size_t j = 0; j < backendCount(); j++) {
            const ExternalDisplayBackend *backend = backendAtIndex(j);
            if (!backend || !backend->disableDisplay) {
                continue;
            }
            if (backend->disableDisplay(displayID)) {
                disabled = YES;
                break;
            }
        }
        
        if (disabled) {
            result.disabled++;
        } else {
            result.failed++;
            LogError(@"failed to disable external display %u", displayID);
        }
    }
    
    finalizeAllBackends();
    externalDisplaysDisabled = (result.disabled > 0);
    
    return result;
}

ExternalDisplayRestoreResult ExternalDisplaysRestore(void) {
    ensureBackendsInitialized();
    ExternalDisplayRestoreResult result = {
        .ok = YES,
        .hadBackups = NO,
        .restored = 0
    };
    
    BOOL hasBackups = externalDisplaysDisabled;
    for (size_t i = 0; i < backendCount(); i++) {
        const ExternalDisplayBackend *backend = backendAtIndex(i);
        if (backend && backend->hasBackups && backend->hasBackups()) {
            hasBackups = YES;
        }
    }
    result.hadBackups = hasBackups;
    
    if (!hasBackups) {
        externalDisplaysDisabled = NO;
        return result;
    }
    
    for (size_t i = 0; i < backendCount(); i++) {
        const ExternalDisplayBackend *backend = backendAtIndex(i);
        if (backend && backend->restoreAll) {
            result.restored += backend->restoreAll();
        }
    }

    BOOL remainingBackups = NO;
    for (size_t i = 0; i < backendCount(); i++) {
        const ExternalDisplayBackend *backend = backendAtIndex(i);
        if (backend && backend->hasBackups && backend->hasBackups()) {
            remainingBackups = YES;
            break;
        }
    }

    externalDisplaysDisabled = remainingBackups;
    if (remainingBackups) {
        result.ok = NO;
    }

    return result;
}

BOOL ExternalDisplaysAreDisabled(void) {
    ensureBackendsInitialized();
    return externalDisplaysDisabled;
}

NSDictionary *ExternalDisplaysCopyState(void) {
    ensureBackendsInitialized();
    NSMutableDictionary *state = [NSMutableDictionary dictionary];
    
    for (size_t i = 0; i < backendCount(); i++) {
        const ExternalDisplayBackend *backend = backendAtIndex(i);
        if (!backend || !backend->copyState) {
            continue;
        }
        
        NSDictionary *backendState = backend->copyState();
        if (!backendState || backendState.count == 0) {
            continue;
        }
        
        NSString *key = backendKey(backend);
        if (key) {
            state[key] = backendState;
        }
    }
    
    if (state.count == 0) {
        return nil;
    }
    
    return [state copy];
}

ExternalDisplayRestoreResult ExternalDisplaysRestoreFromState(NSDictionary *state) {
    ensureBackendsInitialized();
    ExternalDisplayRestoreResult result = {
        .ok = YES,
        .hadBackups = (state != nil),
        .restored = 0
    };
    
    if (!state) {
        result.ok = NO;
        return result;
    }
    
    for (size_t i = 0; i < backendCount(); i++) {
        const ExternalDisplayBackend *backend = backendAtIndex(i);
        if (!backend || !backend->restoreFromState) {
            continue;
        }
        
        NSString *key = backendKey(backend);
        NSDictionary *backendState = key ? state[key] : nil;
        if (![backendState isKindOfClass:[NSDictionary class]]) {
            continue;
        }
        
        size_t restored = 0;
        BOOL ok = backend->restoreFromState(backendState, &restored);
        if (!ok) {
            result.ok = NO;
        }
        result.restored += restored;
    }
    
    externalDisplaysDisabled = NO;
    return result;
}
