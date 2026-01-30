#import "monitor.h"
#import "logging.h"
#import "lid_sensor.h"
#import "brightness.h"
#import "caffeinate.h"
#import "external_display.h"
#import "recovery_state.h"
#import <IOKit/IOKitLib.h>
#import <IOKit/pwr_mgt/IOPMLib.h>
#import <IOKit/IOMessage.h>

// Default angle threshold where dimming begins (degrees).
const int MonitorDefaultThreshold = 30;
// Polling interval for lid sensor reads (milliseconds).
const int MonitorDefaultIntervalMs = 300;
// Angle below which the lid is considered fully closed (degrees).
const int MonitorFullCloseAngle = 10;
// Number of stable samples before dimming on partial close.
const int MonitorPartialStabilitySamples = 2;
// Grace period after close/open to avoid flicker (seconds).
const NSTimeInterval MonitorPostCloseGraceSeconds = 1.0;
// Grace period after wake to avoid immediate dim (seconds).
const NSTimeInterval MonitorPostWakeGraceSeconds = 1.0;

typedef NS_ENUM(NSInteger, LidState) {
    LidStateUnknown = 0,
    LidStateFullyClosed,
    LidStatePartiallyClosed,
    LidStateOpen
};

typedef struct {
    float savedBrightness;
    BOOL brightnessLowered;
    int lastAngle;
    int belowThresholdStreak;
    NSTimeInterval lastFullCloseAt;
    NSTimeInterval lastWakeAt;
    BOOL systemSleeping;
} MonitorState;

static MonitorState monitorState = {
    .savedBrightness = -1.0f,
    .brightnessLowered = NO,
    .lastAngle = -1,
    .belowThresholdStreak = 0,
    .lastFullCloseAt = 0.0,
    .lastWakeAt = 0.0,
    .systemSleeping = NO
};

static NSObject *stateLock = nil;

static io_connect_t powerRootPort = 0;
static IONotificationPortRef powerNotifyPort = NULL;
static io_object_t powerNotifier = 0;

static LidState lidStateForAngle(int angle, int threshold) {
    if (angle == LID_ANGLE_ERROR) {
        return LidStateUnknown;
    }
    if (angle < MonitorFullCloseAngle) {
        return LidStateFullyClosed;
    }
    if (angle < threshold) {
        return LidStatePartiallyClosed;
    }
    return LidStateOpen;
}

static void persistRecoveryStateLocked(void) {
    RecoveryState state = {
        .pendingBrightnessRestore = (monitorState.brightnessLowered &&
                                     monitorState.savedBrightness >= 0.0f),
        .savedBrightness = monitorState.savedBrightness,
        .pendingExternalRestore = ExternalDisplaysAreDisabled()
    };
    
    NSDictionary *externalState = nil;
    if (state.pendingExternalRestore) {
        externalState = ExternalDisplaysCopyState();
    }
    
    if (state.pendingBrightnessRestore || state.pendingExternalRestore) {
        if (!RecoveryStateSave(&state, externalState)) {
            LogError(@"failed to persist recovery state");
        }
    } else {
        RecoveryStateClear();
    }
}

static void recoverStateIfNeeded(void) {
    RecoveryState state = {0};
    NSDictionary *externalState = nil;
    if (!RecoveryStateLoad(&state, &externalState)) {
        return;
    }
    
    LogInfo(@"recovery state detected, attempting restore");
    
    if (state.pendingBrightnessRestore && state.savedBrightness >= 0.0f) {
        if (BrightnessSet(state.savedBrightness)) {
            LogInfo(@"restored brightness to %0.2f", state.savedBrightness);
            state.pendingBrightnessRestore = NO;
            state.savedBrightness = -1.0f;
        } else {
            LogError(@"failed to restore brightness during recovery");
        }
    }
    
    if (state.pendingExternalRestore) {
        ExternalDisplayRestoreResult result = {0};
        if (externalState) {
            result = ExternalDisplaysRestoreFromState(externalState);
        } else {
            result = ExternalDisplaysRestore();
        }
        
        if (result.ok && result.restored > 0) {
            LogInfo(@"restored %zu external displays", result.restored);
            state.pendingExternalRestore = NO;
        } else if (result.ok && !externalState) {
            LogInfo(@"external display recovery requested with no state");
            state.pendingExternalRestore = NO;
        } else {
            LogError(@"failed to restore external displays during recovery");
        }
    }
    
    if (state.pendingBrightnessRestore || state.pendingExternalRestore) {
        RecoveryStateSave(&state, externalState);
    } else {
        RecoveryStateClear();
    }
}

static BOOL restoreBrightnessLocked(BOOL logRestore) {
    BOOL restored = YES;
    if (monitorState.brightnessLowered && monitorState.savedBrightness >= 0.0f) {
        if (logRestore) {
            LogInfo(@"restoring brightness to %0.2f", monitorState.savedBrightness);
        }
        if (BrightnessSet(monitorState.savedBrightness)) {
            monitorState.brightnessLowered = NO;
            monitorState.savedBrightness = -1.0f;
        } else {
            restored = NO;
            if (logRestore) {
                LogError(@"failed to restore brightness");
            }
        }
    }
    
    if (!CaffeinateStop()) {
        LogError(@"failed to stop caffeinate session");
    }
    
    return restored;
}

static void logDisableResult(ExternalDisplayDisableResult result) {
    if (!result.ok) {
        LogError(@"external display disable failed");
        return;
    }
    
    if (result.failed > 0) {
        LogError(@"external display disable failed for %zu displays", result.failed);
    } else if (result.totalExternal > 0 && result.disabled == 0) {
        LogInfo(@"no external displays were disabled");
    }
}

static void handleFullyClosedLocked(NSTimeInterval now) {
    monitorState.lastFullCloseAt = now;
    monitorState.belowThresholdStreak = 0;
    
    restoreBrightnessLocked(YES);
    ExternalDisplaysRestore();
    persistRecoveryStateLocked();
}

static void handlePartiallyClosedLocked(int angle, NSTimeInterval now) {
    NSTimeInterval sinceClose = (monitorState.lastFullCloseAt > 0.0)
        ? (now - monitorState.lastFullCloseAt)
        : MonitorPostCloseGraceSeconds;
    NSTimeInterval sinceWake = (monitorState.lastWakeAt > 0.0)
        ? (now - monitorState.lastWakeAt)
        : MonitorPostWakeGraceSeconds;
    BOOL graceActive = (sinceClose < MonitorPostCloseGraceSeconds) ||
                       (sinceWake < MonitorPostWakeGraceSeconds);
    
    if (monitorState.brightnessLowered) {
        monitorState.belowThresholdStreak = 0;
        if (!ExternalDisplaysAreDisabled()) {
            ExternalDisplayDisableResult disableResult = ExternalDisplaysDisable();
            logDisableResult(disableResult);
            persistRecoveryStateLocked();
        }
        return;
    }
    
    if (graceActive) {
        monitorState.belowThresholdStreak = 0;
        return;
    }
    
    BOOL notOpening = (monitorState.lastAngle == -1) ? YES : (angle <= monitorState.lastAngle);
    if (notOpening) {
        monitorState.belowThresholdStreak++;
    } else {
        monitorState.belowThresholdStreak = 0;
    }
    
    if (monitorState.belowThresholdStreak >= MonitorPartialStabilitySamples) {
        ExternalDisplayDisableResult disableResult = ExternalDisplaysDisable();
        logDisableResult(disableResult);
        
        float currentBrightness = BrightnessGet();
        if (currentBrightness < 0.0f) {
            LogError(@"failed to read brightness");
            return;
        }
        
        if (!BrightnessSet(0.0f)) {
            LogError(@"failed to dim display");
            return;
        }
        
        monitorState.savedBrightness = currentBrightness;
        monitorState.brightnessLowered = YES;
        LogInfo(@"dimming display to 0.0f");
        
        if (!CaffeinateStart()) {
            LogError(@"failed to start caffeinate session");
        }
        
        persistRecoveryStateLocked();
    }
}

static void handleOpenLocked(void) {
    monitorState.belowThresholdStreak = 0;
    restoreBrightnessLocked(YES);
    ExternalDisplaysRestore();
    persistRecoveryStateLocked();
}

static void powerCallback(void *refCon, io_service_t service, natural_t messageType, void *messageArgument) {
    (void)refCon;
    (void)service;
    
    switch (messageType) {
        case kIOMessageCanSystemSleep:
            IOAllowPowerChange(powerRootPort, (long)messageArgument);
            break;
        case kIOMessageSystemWillSleep: {
            NSTimeInterval now = [NSDate timeIntervalSinceReferenceDate];
            @synchronized(stateLock) {
                monitorState.systemSleeping = YES;
                monitorState.lastFullCloseAt = now;
                monitorState.lastWakeAt = 0.0;
                monitorState.lastAngle = -1;
                monitorState.belowThresholdStreak = 0;
                restoreBrightnessLocked(NO);
                ExternalDisplaysRestore();
                persistRecoveryStateLocked();
            }
            IOAllowPowerChange(powerRootPort, (long)messageArgument);
            break;
        }
        case kIOMessageSystemHasPoweredOn: {
            NSTimeInterval now = [NSDate timeIntervalSinceReferenceDate];
            @synchronized(stateLock) {
                monitorState.systemSleeping = NO;
                monitorState.lastWakeAt = now;
                monitorState.lastFullCloseAt = now;
                monitorState.lastAngle = -1;
                monitorState.belowThresholdStreak = 0;
            }
            break;
        }
        default:
            break;
    }
}

static void registerPowerNotifications(void) {
    powerRootPort = IORegisterForSystemPower(
        NULL,
        &powerNotifyPort,
        powerCallback,
        &powerNotifier
    );
    
    if (powerRootPort == 0 || powerNotifyPort == NULL) {
        LogError(@"failed to register power notifications");
        return;
    }
    
    CFRunLoopAddSource(
        CFRunLoopGetCurrent(),
        IONotificationPortGetRunLoopSource(powerNotifyPort),
        kCFRunLoopCommonModes
    );
    
    CFRunLoopRun();
}

static void startPowerMonitor(void) {
    NSThread *powerThread = [[NSThread alloc] initWithBlock:^{
        @autoreleasepool {
            registerPowerNotifications();
        }
    }];
    powerThread.name = @"lidoff.power";
    [powerThread start];
}

void MonitorRun(const MonitorConfig *config, volatile sig_atomic_t *shouldRunFlag) {
    if (!config || !shouldRunFlag) {
        return;
    }
    
    stateLock = [NSObject new];
    monitorState = (MonitorState){
        .savedBrightness = -1.0f,
        .brightnessLowered = NO,
        .lastAngle = -1,
        .belowThresholdStreak = 0,
        .lastFullCloseAt = 0.0,
        .lastWakeAt = 0.0,
        .systemSleeping = NO
    };
    recoverStateIfNeeded();
    startPowerMonitor();
    
    NSTimeInterval interval = config->intervalMs / 1000.0;
    
    while (*shouldRunFlag) {
        @autoreleasepool {
            int angle = LidSensorGetAngle();
            if (angle == LID_ANGLE_ERROR) {
                [NSThread sleepForTimeInterval:interval];
                continue;
            }
            
            LogDebug(@"angle %d°", angle);
            NSTimeInterval now = [NSDate timeIntervalSinceReferenceDate];
            BOOL skipThisCycle = NO;
            
            @synchronized(stateLock) {
                if (monitorState.systemSleeping) {
                    skipThisCycle = YES;
                } else {
                    LidState state = lidStateForAngle(angle, config->threshold);
                    switch (state) {
                        case LidStateFullyClosed:
                            handleFullyClosedLocked(now);
                            break;
                        case LidStatePartiallyClosed:
                            handlePartiallyClosedLocked(angle, now);
                            break;
                        case LidStateOpen:
                            handleOpenLocked();
                            break;
                        case LidStateUnknown:
                        default:
                            break;
                    }
                }
                
                if (!skipThisCycle) {
                    monitorState.lastAngle = angle;
                }
            }
            
            [NSThread sleepForTimeInterval:interval];
            if (skipThisCycle) {
                continue;
            }
        }
    }
    
    @synchronized(stateLock) {
        restoreBrightnessLocked(NO);
        ExternalDisplaysRestore();
        persistRecoveryStateLocked();
    }
}
