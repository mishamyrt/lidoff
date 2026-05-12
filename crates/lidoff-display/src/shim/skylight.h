#ifndef LIDOFF_EXTERNAL_DISPLAY_H
#define LIDOFF_EXTERNAL_DISPLAY_H

#include <CoreGraphics/CoreGraphics.h>
#include <stddef.h>
#include <stdint.h>

uint8_t SkylightPrepare(size_t display_count);
uint8_t SkylightFinalize(void);
void SkylightClearBackups(void);
uint8_t SkylightCaptureDisplay(CGDirectDisplayID display_id);
uint8_t SkylightDisableDisplay(CGDirectDisplayID display_id);
size_t SkylightRestoreAll(void);
size_t SkylightBackupCount(void);
size_t SkylightCopyState(CGDirectDisplayID *display_ids, size_t capacity);
size_t SkylightRestoreFromState(const CGDirectDisplayID *display_ids, size_t count);

#endif /* LIDOFF_EXTERNAL_DISPLAY_H */
