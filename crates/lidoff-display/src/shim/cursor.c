#include "cursor.h"

#include <CoreGraphics/CoreGraphics.h>
#include <stdbool.h>

static bool cursorLocked = false;

uint8_t CursorLock(void) {
    if (cursorLocked) {
        return 1;
    }

    if (CGAssociateMouseAndMouseCursorPosition(false) != kCGErrorSuccess) {
        return 0;
    }

    cursorLocked = true;
    return 1;
}

uint8_t CursorUnlock(void) {
    if (CGAssociateMouseAndMouseCursorPosition(true) != kCGErrorSuccess) {
        return 0;
    }

    cursorLocked = false;
    return 1;
}

uint8_t CursorIsLocked(void) {
    return cursorLocked ? 1 : 0;
}
