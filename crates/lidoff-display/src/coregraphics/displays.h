#ifndef LIDOFF_DISPLAYS_H
#define LIDOFF_DISPLAYS_H

#include <CoreGraphics/CoreGraphics.h>
#include <stddef.h>
#include <stdint.h>

uint8_t DisplaysListOnline(CGDirectDisplayID *display_ids, size_t capacity, size_t *count_out);
uint8_t DisplayIsBuiltin(CGDirectDisplayID display_id);

#endif /* LIDOFF_DISPLAYS_H */
