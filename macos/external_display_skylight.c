#include "external_display.h"

#include <CoreGraphics/CGDisplayConfiguration.h>
#include <dlfcn.h>
#include <stdbool.h>
#include <stdlib.h>

typedef CGError (*SLSConfigureDisplayEnabledFunc)(CGDisplayConfigRef, CGDirectDisplayID, Boolean);

static CGDirectDisplayID *skylight_backups = NULL;
static size_t skylight_backup_count = 0;
static size_t skylight_backup_capacity = 0;
static bool skylight_loaded = false;
static bool skylight_available = false;
static void *skylight_handle = NULL;
static SLSConfigureDisplayEnabledFunc SLSConfigureDisplayEnabledPtr = NULL;

static void clearSkylightBackups(void) {
  if (skylight_backups == NULL) {
    skylight_backup_count = 0;
    skylight_backup_capacity = 0;
    return;
  }

  free(skylight_backups);
  skylight_backups = NULL;
  skylight_backup_count = 0;
  skylight_backup_capacity = 0;
}

void ExternalDisplaySkylightClearBackups(void) { clearSkylightBackups(); }

uint8_t ExternalDisplaySkylightPrepare(size_t display_count) {
  clearSkylightBackups();
  if (display_count == 0) {
    return 1;
  }

  skylight_backups = calloc(display_count, sizeof(*skylight_backups));
  if (skylight_backups == NULL) {
    return 0;
  }

  skylight_backup_capacity = display_count;
  return 1;
}

void ExternalDisplaySkylightFinalize(void) {
  if (skylight_backup_count == 0) {
    clearSkylightBackups();
  }
}

size_t ExternalDisplaySkylightBackupCount(void) { return skylight_backup_count; }

static void loadSkylight(void) {
  if (skylight_loaded) {
    return;
  }

  skylight_loaded = true;
  skylight_handle =
      dlopen("/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight", RTLD_NOW);
  if (skylight_handle == NULL) {
    return;
  }

  SLSConfigureDisplayEnabledPtr =
      (SLSConfigureDisplayEnabledFunc)dlsym(skylight_handle, "SLSConfigureDisplayEnabled");
  skylight_available = (SLSConfigureDisplayEnabledPtr != NULL);
}

static bool skylightSetDisplayEnabled(CGDirectDisplayID display_id, bool enabled) {
  loadSkylight();
  if (!skylight_available) {
    return false;
  }

  CGDisplayConfigRef config = NULL;
  CGError err = CGBeginDisplayConfiguration(&config);
  if (err != kCGErrorSuccess || config == NULL) {
    return false;
  }

  err = SLSConfigureDisplayEnabledPtr(config, display_id, enabled ? 1 : 0);
  if (err != kCGErrorSuccess) {
    CGCancelDisplayConfiguration(config);
    return false;
  }

  err = CGCompleteDisplayConfiguration(config, kCGConfigureForSession);
  if (err != kCGErrorSuccess) {
    CGCancelDisplayConfiguration(config);
    return false;
  }

  return true;
}

uint8_t ExternalDisplaySkylightDisableDisplay(CGDirectDisplayID display_id) {
  if (!skylightSetDisplayEnabled(display_id, false)) {
    return 0;
  }

  if (skylight_backups == NULL || skylight_backup_count >= skylight_backup_capacity) {
    skylightSetDisplayEnabled(display_id, true);
    return 0;
  }

  skylight_backups[skylight_backup_count++] = display_id;
  return 1;
}

size_t ExternalDisplaySkylightRestoreAll(void) {
  if (skylight_backups == NULL || skylight_backup_count == 0) {
    return 0;
  }

  size_t restored = 0;
  size_t remaining = 0;
  for (size_t i = 0; i < skylight_backup_count; i++) {
    CGDirectDisplayID display_id = skylight_backups[i];
    if (skylightSetDisplayEnabled(display_id, true)) {
      restored++;
      continue;
    }

    skylight_backups[remaining++] = display_id;
  }

  skylight_backup_count = remaining;
  if (skylight_backup_count == 0) {
    clearSkylightBackups();
  }

  return restored;
}

size_t ExternalDisplaySkylightCopyState(CGDirectDisplayID *display_ids, size_t capacity) {
  if (display_ids == NULL || capacity == 0) {
    return 0;
  }

  size_t copied = skylight_backup_count < capacity ? skylight_backup_count : capacity;
  for (size_t i = 0; i < copied; i++) {
    display_ids[i] = skylight_backups[i];
  }

  return copied;
}

size_t ExternalDisplaySkylightRestoreFromState(const CGDirectDisplayID *display_ids, size_t count) {
  if (display_ids == NULL && count > 0) {
    clearSkylightBackups();
    return 0;
  }

  size_t restored = 0;
  for (size_t i = 0; i < count; i++) {
    if (skylightSetDisplayEnabled(display_ids[i], true)) {
      restored++;
    }
  }

  clearSkylightBackups();
  return restored;
}
