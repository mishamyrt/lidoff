#ifndef LIDOFF_EXTERNAL_DISPLAY_H
#define LIDOFF_EXTERNAL_DISPLAY_H

#include <CoreGraphics/CoreGraphics.h>
#include <stddef.h>
#include <stdint.h>

uint8_t SkylightFinalize(void);
uint8_t SkylightDisableDisplay(CGDirectDisplayID display_id);
size_t SkylightRestoreFromState(const CGDirectDisplayID *display_ids, size_t count);

#endif /* LIDOFF_EXTERNAL_DISPLAY_H */
