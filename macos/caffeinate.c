#include "caffeinate.h"

#include <IOKit/pwr_mgt/IOPMLib.h>
#include <stdbool.h>

static IOPMAssertionID assertion_id = 0;
static bool caffeinate_active = false;

uint8_t CaffeinateStart(void) {
  if (caffeinate_active) {
    return 1;
  }

  IOReturn result = IOPMAssertionCreateWithName(
      kIOPMAssertionTypePreventUserIdleDisplaySleep, kIOPMAssertionLevelOn,
      CFSTR("lidoff: lid partially closed"), &assertion_id);
  if (result == kIOReturnSuccess) {
    caffeinate_active = true;
    return 1;
  }

  return 0;
}

uint8_t CaffeinateStop(void) {
  if (!caffeinate_active) {
    return 1;
  }

  IOReturn result = IOPMAssertionRelease(assertion_id);
  if (result == kIOReturnSuccess) {
    assertion_id = 0;
    caffeinate_active = false;
    return 1;
  }

  return 0;
}
