#ifndef EXTERNAL_DISPLAY_H
#define EXTERNAL_DISPLAY_H

#include <CoreGraphics/CoreGraphics.h>
#include <stddef.h>
#include <stdint.h>

typedef struct {
  uint8_t ok;
  uint8_t already_disabled;
  size_t total_external;
  size_t disabled;
  size_t failed;
} ExternalDisplayDisableResult;

typedef struct {
  uint8_t ok;
  uint8_t had_backups;
  size_t restored;
} ExternalDisplayRestoreResult;

typedef struct {
  CGDirectDisplayID display_id;
  uint16_t brightness;
  uint16_t contrast;
  uint8_t has_brightness;
  uint8_t has_contrast;
  uint32_t gamma_sample_count;
  const float *gamma_red;
  const float *gamma_green;
  const float *gamma_blue;
} ExternalDisplayGammaBackupView;

uint8_t ExternalDisplayGetOnlineDisplays(CGDirectDisplayID *display_ids,
                                         size_t capacity, size_t *count_out);
uint8_t ExternalDisplayIsBuiltin(CGDirectDisplayID display_id);

uint8_t ExternalDisplaySkylightPrepare(size_t display_count);
void ExternalDisplaySkylightFinalize(void);
void ExternalDisplaySkylightClearBackups(void);
uint8_t ExternalDisplaySkylightDisableDisplay(CGDirectDisplayID display_id);
size_t ExternalDisplaySkylightRestoreAll(void);
size_t ExternalDisplaySkylightBackupCount(void);
size_t ExternalDisplaySkylightCopyState(CGDirectDisplayID *display_ids,
                                        size_t capacity);
size_t
ExternalDisplaySkylightRestoreFromState(const CGDirectDisplayID *display_ids,
                                        size_t count);

uint8_t ExternalDisplayGammaPrepare(size_t display_count);
void ExternalDisplayGammaFinalize(void);
void ExternalDisplayGammaClearBackups(void);
uint8_t ExternalDisplayGammaDisableDisplay(CGDirectDisplayID display_id);
size_t ExternalDisplayGammaRestoreAll(void);
size_t ExternalDisplayGammaBackupCount(void);
uint8_t
ExternalDisplayGammaCopyStateView(size_t index,
                                  ExternalDisplayGammaBackupView *backup_out);
size_t ExternalDisplayGammaRestoreFromState(
    const ExternalDisplayGammaBackupView *backups, size_t count);

#endif /* EXTERNAL_DISPLAY_H */
