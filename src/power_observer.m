//
//  power_observer.m
//  lidoff - IOKit system power notification observer
//

#import "power_observer.h"
#import "logging.h"
#import <IOKit/IOKitLib.h>
#import <IOKit/pwr_mgt/IOPMLib.h>
#import <IOKit/IOMessage.h>

static io_connect_t powerRootPort = 0;
static IONotificationPortRef powerNotifyPort = NULL;
static io_object_t powerNotifier = 0;

typedef struct {
    PowerObserverWillSleepHandler willSleep;
    PowerObserverDidWakeHandler didWake;
} PowerObserverContext;

static PowerObserverContext *observerContext = nil;

static void powerCallback(void *refCon, io_service_t service, natural_t messageType, void *messageArgument) {
    (void)refCon;
    (void)service;

    switch (messageType) {
        case kIOMessageCanSystemSleep:
            IOAllowPowerChange(powerRootPort, (long)messageArgument);
            break;
        case kIOMessageSystemWillSleep:
            if (observerContext && observerContext->willSleep) {
                observerContext->willSleep();
            }
            IOAllowPowerChange(powerRootPort, (long)messageArgument);
            break;
        case kIOMessageSystemHasPoweredOn:
            if (observerContext && observerContext->didWake) {
                observerContext->didWake();
            }
            break;
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

void PowerObserverStart(PowerObserverWillSleepHandler willSleep,
                        PowerObserverDidWakeHandler didWake) {
    observerContext = malloc(sizeof(PowerObserverContext));
    observerContext->willSleep = [willSleep copy];
    observerContext->didWake = [didWake copy];

    NSThread *powerThread = [[NSThread alloc] initWithBlock:^{
        @autoreleasepool {
            registerPowerNotifications();
        }
    }];
    powerThread.name = @"lidoff.power";
    [powerThread start];
}
