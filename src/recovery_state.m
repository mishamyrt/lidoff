#import "recovery_state.h"

static NSString *const kRecoveryStateDirectory = @"~/Library/Caches/co.myrt.lidoff";
static NSString *const kRecoveryStateFilename = @"state.plist";

static NSString *const kStateVersionKey = @"version";
static NSString *const kPendingBrightnessKey = @"pendingBrightnessRestore";
static NSString *const kSavedBrightnessKey = @"savedBrightness";
static NSString *const kPendingExternalKey = @"pendingExternalRestore";
static NSString *const kExternalStateKey = @"externalDisplayState";

NSString *RecoveryStatePath(void) {
    NSString *dir = [kRecoveryStateDirectory stringByExpandingTildeInPath];
    return [dir stringByAppendingPathComponent:kRecoveryStateFilename];
}

static NSURL *recoveryStateURL(void) {
    return [NSURL fileURLWithPath:RecoveryStatePath()];
}

BOOL RecoveryStateLoad(RecoveryState *stateOut, NSDictionary **externalStateOut) {
    if (!stateOut) {
        return NO;
    }
    
    stateOut->pendingBrightnessRestore = NO;
    stateOut->savedBrightness = -1.0f;
    stateOut->pendingExternalRestore = NO;
    
    NSString *path = RecoveryStatePath();
    NSData *data = [NSData dataWithContentsOfFile:path];
    if (!data) {
        return NO;
    }
    
    NSPropertyListFormat format = NSPropertyListBinaryFormat_v1_0;
    NSError *error = nil;
    id plist = [NSPropertyListSerialization propertyListWithData:data
                                                         options:NSPropertyListImmutable
                                                          format:&format
                                                           error:&error];
    if (![plist isKindOfClass:[NSDictionary class]]) {
        return NO;
    }
    
    NSDictionary *dict = (NSDictionary *)plist;
    NSNumber *pendingBrightness = dict[kPendingBrightnessKey];
    NSNumber *savedBrightness = dict[kSavedBrightnessKey];
    NSNumber *pendingExternal = dict[kPendingExternalKey];
    
    if ([pendingBrightness isKindOfClass:[NSNumber class]]) {
        stateOut->pendingBrightnessRestore = pendingBrightness.boolValue;
    }
    if ([savedBrightness isKindOfClass:[NSNumber class]]) {
        stateOut->savedBrightness = savedBrightness.floatValue;
    }
    if ([pendingExternal isKindOfClass:[NSNumber class]]) {
        stateOut->pendingExternalRestore = pendingExternal.boolValue;
    }
    
    if (externalStateOut) {
        NSDictionary *externalState = dict[kExternalStateKey];
        if ([externalState isKindOfClass:[NSDictionary class]]) {
            *externalStateOut = externalState;
        } else {
            *externalStateOut = nil;
        }
    }
    
    return YES;
}

BOOL RecoveryStateSave(const RecoveryState *state, NSDictionary *externalState) {
    if (!state) {
        return NO;
    }
    
    NSMutableDictionary *dict = [NSMutableDictionary dictionary];
    dict[kStateVersionKey] = @"1";
    dict[kPendingBrightnessKey] = @(state->pendingBrightnessRestore);
    dict[kSavedBrightnessKey] = @(state->savedBrightness);
    dict[kPendingExternalKey] = @(state->pendingExternalRestore);
    if (externalState) {
        dict[kExternalStateKey] = externalState;
    }
    
    NSData *data = [NSPropertyListSerialization dataWithPropertyList:dict
                                                              format:NSPropertyListBinaryFormat_v1_0
                                                             options:0
                                                               error:nil];
    if (!data) {
        return NO;
    }
    
    NSString *dir = [kRecoveryStateDirectory stringByExpandingTildeInPath];
    NSFileManager *fm = [NSFileManager defaultManager];
    if (![fm fileExistsAtPath:dir]) {
        NSError *error = nil;
        if (![fm createDirectoryAtPath:dir
           withIntermediateDirectories:YES
                            attributes:nil
                                 error:&error]) {
            return NO;
        }
    }
    
    return [data writeToURL:recoveryStateURL() atomically:YES];
}

void RecoveryStateClear(void) {
    NSString *path = RecoveryStatePath();
    [[NSFileManager defaultManager] removeItemAtPath:path error:nil];
}
