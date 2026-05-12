#include "displays.h"

#include <CoreGraphics/CoreGraphics.h>
#include <limits.h>

uint8_t DisplaysListOnline(CGDirectDisplayID *display_ids,
                           size_t capacity,
                           size_t *count_out) {
    if (count_out == NULL) {
        return 0;
    }

    CGDisplayCount display_count = 0;
    CGDisplayCount max_displays = capacity > UINT_MAX ? UINT_MAX : (CGDisplayCount)capacity;
    CGError err = CGGetOnlineDisplayList(max_displays, display_ids, &display_count);
    *count_out = (size_t)display_count;
    return err == kCGErrorSuccess ? 1 : 0;
}

uint8_t DisplayIsBuiltin(CGDirectDisplayID display_id) {
    return CGDisplayIsBuiltin(display_id) ? 1 : 0;
}
