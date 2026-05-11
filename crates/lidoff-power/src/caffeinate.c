#include "caffeinate.h"

#include <IOKit/pwr_mgt/IOPMLib.h>

static IOPMAssertionID assertion_id = 0;
static bool caffeinate_active = false;

#define CAFFEINATE_ASSERTION_NAME CFSTR("lidoff")
#define CAFFEINATE_STATUS_SUCCESS 0
#define CAFFEINATE_STATUS_FAILURE 1
#define CAFFEINATE_ACTIVE 2

uint8_t CaffeinateStart(void) {
    if (caffeinate_active) {
        return CAFFEINATE_ACTIVE;
    }

    IOReturn result =
        IOPMAssertionCreateWithName(kIOPMAssertionTypePreventUserIdleDisplaySleep,
                                    kIOPMAssertionLevelOn, CFSTR("lidoff"), &assertion_id);
    if (result == kIOReturnSuccess) {
        caffeinate_active = true;
        return CAFFEINATE_STATUS_SUCCESS;
    }

    return CAFFEINATE_STATUS_FAILURE;
}

uint8_t CaffeinateStop(void) {
    if (!caffeinate_active) {
        return CAFFEINATE_ACTIVE;
    }

    IOReturn result = IOPMAssertionRelease(assertion_id);
    if (result == kIOReturnSuccess) {
        assertion_id = 0;
        caffeinate_active = false;
        return CAFFEINATE_STATUS_SUCCESS;
    }

    return CAFFEINATE_STATUS_FAILURE;
}
