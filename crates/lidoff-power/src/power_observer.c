#include "power_observer.h"

#include <stdbool.h>

#include <CoreFoundation/CoreFoundation.h>
#include <IOKit/IOKitLib.h>
#include <IOKit/IOMessage.h>
#include <IOKit/pwr_mgt/IOPMLib.h>

typedef struct {
    PowerObserverCallback will_sleep;
    PowerObserverCallback did_wake;
    void *context;
} PowerObserverContext;

static io_connect_t power_root_port = 0;
static IONotificationPortRef power_notify_port = NULL;
static io_object_t power_notifier = 0;
static PowerObserverContext observer_context = {0};

#define POWER_NOTIFIER_NAME "lidoff.power"
#define POWER_NOTIFIER_STATUS_SUCCESS 0
#define POWER_NOTIFIER_STATUS_FAILURE 1

static void cleanupPowerNotifications(void) {
    if (power_notifier != 0) {
        IOObjectRelease(power_notifier);
        power_notifier = 0;
    }

    if (power_notify_port != NULL) {
        IONotificationPortDestroy(power_notify_port);
        power_notify_port = NULL;
    }

    if (power_root_port != 0) {
        IOServiceClose(power_root_port);
        power_root_port = 0;
    }
}

static void powerCallback(void *ref_con,
                          io_service_t service,
                          natural_t message_type,
                          void *message_argument) {
    (void)ref_con;
    (void)service;

    switch (message_type) {
        case kIOMessageCanSystemSleep:
            IOAllowPowerChange(power_root_port, (long)message_argument);
            break;
        case kIOMessageSystemWillSleep:
            if (observer_context.will_sleep != NULL) {
                observer_context.will_sleep(observer_context.context);
            }
            IOAllowPowerChange(power_root_port, (long)message_argument);
            break;
        case kIOMessageSystemHasPoweredOn:
            if (observer_context.did_wake != NULL) {
                observer_context.did_wake(observer_context.context);
            }
            break;
        default:
            break;
    }
}

uint8_t PowerObserverRunLoop(PowerObserverCallback will_sleep,
                             PowerObserverCallback did_wake,
                             void *context,
                             PowerObserverStartupCallback startup_callback,
                             void *startup_context) {
    observer_context.will_sleep = will_sleep;
    observer_context.did_wake = did_wake;
    observer_context.context = context;

    power_root_port =
        IORegisterForSystemPower(NULL, &power_notify_port, powerCallback, &power_notifier);
    if (power_root_port == 0 || power_notify_port == NULL) {
        cleanupPowerNotifications();
        startup_callback(POWER_NOTIFIER_STATUS_FAILURE, startup_context);
        return POWER_NOTIFIER_STATUS_FAILURE;
    }

    CFRunLoopAddSource(CFRunLoopGetCurrent(),
                       IONotificationPortGetRunLoopSource(power_notify_port),
                       kCFRunLoopCommonModes);
    startup_callback(POWER_NOTIFIER_STATUS_SUCCESS, startup_context);
    CFRunLoopRun();
    cleanupPowerNotifications();
    return POWER_NOTIFIER_STATUS_SUCCESS;
}
