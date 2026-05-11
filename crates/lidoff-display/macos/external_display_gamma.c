#include "external_display.h"

#include <CoreGraphics/CGDisplayConfiguration.h>
#include <CoreGraphics/CoreGraphics.h>
#include <IOKit/IOKitLib.h>
#include <IOKit/i2c/IOI2CInterface.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#define DDC_ADDRESS 0x37
#define DDC_HEADER 0x51
#define DDC_CMD_SET_VCP 0x03
#define DDC_CMD_GET_VCP 0x01
#define DDC_LEN_SET_VCP 0x84
#define DDC_LEN_GET_VCP 0x82

typedef struct {
  CGDirectDisplayID display_id;
  uint16_t brightness;
  uint16_t contrast;
  bool has_brightness;
  bool has_contrast;
  uint32_t gamma_sample_count;
  float *gamma_red;
  float *gamma_green;
  float *gamma_blue;
} DisplayBackup;

static DisplayBackup *display_backups = NULL;
static size_t display_backup_count = 0;
static size_t display_backup_capacity = 0;

static void resetBackup(DisplayBackup *backup) {
  if (backup == NULL) {
    return;
  }

  free(backup->gamma_red);
  free(backup->gamma_green);
  free(backup->gamma_blue);
  backup->gamma_red = NULL;
  backup->gamma_green = NULL;
  backup->gamma_blue = NULL;
  backup->gamma_sample_count = 0;
}

static void clearBackups(void) {
  if (display_backups == NULL) {
    display_backup_count = 0;
    display_backup_capacity = 0;
    return;
  }

  for (size_t i = 0; i < display_backup_count; i++) {
    resetBackup(&display_backups[i]);
  }

  free(display_backups);
  display_backups = NULL;
  display_backup_count = 0;
  display_backup_capacity = 0;
}

void ExternalDisplayGammaClearBackups(void) { clearBackups(); }

uint8_t ExternalDisplayGammaPrepare(size_t display_count) {
  clearBackups();
  if (display_count == 0) {
    return 1;
  }

  display_backups = calloc(display_count, sizeof(*display_backups));
  if (display_backups == NULL) {
    return 0;
  }

  display_backup_capacity = display_count;
  return 1;
}

void ExternalDisplayGammaFinalize(void) {
  if (display_backup_count == 0) {
    clearBackups();
  }
}

size_t ExternalDisplayGammaBackupCount(void) { return display_backup_count; }

static uint8_t ddcChecksum(const uint8_t *payload, size_t length) {
  uint8_t checksum = (uint8_t)(DDC_ADDRESS << 1);
  for (size_t i = 0; i < length; i++) {
    checksum ^= payload[i];
  }
  return checksum;
}

static IOReturn sendI2CRequest(io_service_t framebuffer,
                               IOI2CRequest *request) {
  io_service_t interface = MACH_PORT_NULL;
  IOReturn status = IOFBCopyI2CInterfaceForBus(framebuffer, 0, &interface);
  if (status != kIOReturnSuccess || interface == MACH_PORT_NULL) {
    return status;
  }

  IOI2CConnectRef connect = NULL;
  status = IOI2CInterfaceOpen(interface, kNilOptions, &connect);
  if (status == kIOReturnSuccess) {
    status = IOI2CSendRequest(connect, kNilOptions, request);
    IOI2CInterfaceClose(connect, kNilOptions);
    if (status == kIOReturnSuccess) {
      status = request->result;
    }
  }

  IOObjectRelease(interface);
  return status;
}

static bool ddcGetVCP(io_service_t framebuffer, uint8_t code,
                      uint16_t *value_out) {
  uint8_t send_buffer[5] = {DDC_HEADER, DDC_LEN_GET_VCP, DDC_CMD_GET_VCP, code,
                            0};
  send_buffer[4] = ddcChecksum(send_buffer, 4);

  uint8_t reply_buffer[11] = {0};

  IOI2CRequest request;
  memset(&request, 0, sizeof(request));
  request.sendTransactionType = kIOI2CSimpleTransactionType;
  request.replyTransactionType = kIOI2CDDCciReplyTransactionType;
  request.sendAddress = DDC_ADDRESS << 1;
  request.replyAddress = DDC_ADDRESS << 1;
  request.sendBytes = (uint32_t)sizeof(send_buffer);
  request.replyBytes = (uint32_t)sizeof(reply_buffer);
  request.sendBuffer = (vm_address_t)send_buffer;
  request.replyBuffer = (vm_address_t)reply_buffer;

  IOReturn status = sendI2CRequest(framebuffer, &request);
  if (status != kIOReturnSuccess) {
    return false;
  }

  if (reply_buffer[2] != 0x02 || reply_buffer[4] != code) {
    return false;
  }

  *value_out = (uint16_t)((reply_buffer[8] << 8) | reply_buffer[9]);
  return true;
}

static bool ddcSetVCP(io_service_t framebuffer, uint8_t code, uint16_t value) {
  uint8_t send_buffer[7] = {
      DDC_HEADER, DDC_LEN_SET_VCP,       DDC_CMD_SET_VCP,
      code,       (uint8_t)(value >> 8), (uint8_t)(value & 0xFF),
      0};
  send_buffer[6] = ddcChecksum(send_buffer, 6);

  IOI2CRequest request;
  memset(&request, 0, sizeof(request));
  request.sendTransactionType = kIOI2CSimpleTransactionType;
  request.replyTransactionType = kIOI2CNoTransactionType;
  request.sendAddress = DDC_ADDRESS << 1;
  request.sendBytes = (uint32_t)sizeof(send_buffer);
  request.sendBuffer = (vm_address_t)send_buffer;

  return sendI2CRequest(framebuffer, &request) == kIOReturnSuccess;
}

static bool backupAndZeroGamma(CGDirectDisplayID display_id,
                               DisplayBackup *backup_out) {
  size_t capacity = CGDisplayGammaTableCapacity(display_id);
  if (capacity == 0 || capacity > UINT32_MAX) {
    return false;
  }

  float *red = calloc(capacity, sizeof(*red));
  float *green = calloc(capacity, sizeof(*green));
  float *blue = calloc(capacity, sizeof(*blue));
  if (red == NULL || green == NULL || blue == NULL) {
    free(red);
    free(green);
    free(blue);
    return false;
  }

  uint32_t sample_count = 0;
  CGError err = CGGetDisplayTransferByTable(display_id, (uint32_t)capacity, red,
                                            green, blue, &sample_count);
  if (err != kCGErrorSuccess || sample_count == 0) {
    free(red);
    free(green);
    free(blue);
    return false;
  }

  float *zeros = calloc((size_t)sample_count, sizeof(*zeros));
  if (zeros == NULL) {
    free(red);
    free(green);
    free(blue);
    return false;
  }

  err = CGSetDisplayTransferByTable(display_id, sample_count, zeros, zeros,
                                    zeros);
  free(zeros);
  if (err != kCGErrorSuccess) {
    free(red);
    free(green);
    free(blue);
    return false;
  }

  backup_out->gamma_sample_count = sample_count;
  backup_out->gamma_red = red;
  backup_out->gamma_green = green;
  backup_out->gamma_blue = blue;
  return true;
}

static bool restoreDisplayFromBackup(const DisplayBackup *backup) {
  if (backup == NULL || !CGDisplayIsOnline(backup->display_id)) {
    return false;
  }

  if (backup->gamma_sample_count > 0 && backup->gamma_red != NULL &&
      backup->gamma_green != NULL && backup->gamma_blue != NULL) {
    CGSetDisplayTransferByTable(backup->display_id, backup->gamma_sample_count,
                                backup->gamma_red, backup->gamma_green,
                                backup->gamma_blue);
  }

  io_service_t framebuffer = CGDisplayIOServicePort(backup->display_id);
  if (framebuffer == MACH_PORT_NULL) {
    return true;
  }

  if (backup->has_brightness) {
    ddcSetVCP(framebuffer, 0x10, backup->brightness);
  }
  if (backup->has_contrast) {
    ddcSetVCP(framebuffer, 0x12, backup->contrast);
  }

  return true;
}

uint8_t ExternalDisplayGammaDisableDisplay(CGDirectDisplayID display_id) {
  DisplayBackup backup = {.display_id = display_id,
                          .brightness = 0,
                          .contrast = 0,
                          .has_brightness = false,
                          .has_contrast = false,
                          .gamma_sample_count = 0,
                          .gamma_red = NULL,
                          .gamma_green = NULL,
                          .gamma_blue = NULL};

  io_service_t framebuffer = CGDisplayIOServicePort(display_id);
  if (framebuffer != MACH_PORT_NULL) {
    uint16_t value = 0;
    if (ddcGetVCP(framebuffer, 0x10, &value)) {
      backup.brightness = value;
      backup.has_brightness = true;
      ddcSetVCP(framebuffer, 0x10, 0);
    }
    if (ddcGetVCP(framebuffer, 0x12, &value)) {
      backup.contrast = value;
      backup.has_contrast = true;
      ddcSetVCP(framebuffer, 0x12, 0);
    }
  }

  backupAndZeroGamma(display_id, &backup);

  if (display_backups == NULL ||
      display_backup_count >= display_backup_capacity) {
    restoreDisplayFromBackup(&backup);
    free(backup.gamma_red);
    free(backup.gamma_green);
    free(backup.gamma_blue);
    return 0;
  }

  display_backups[display_backup_count++] = backup;
  return 1;
}

size_t ExternalDisplayGammaRestoreAll(void) {
  if (display_backups == NULL || display_backup_count == 0) {
    return 0;
  }

  size_t restored = 0;
  size_t remaining = 0;
  for (size_t i = 0; i < display_backup_count; i++) {
    DisplayBackup backup = display_backups[i];
    if (restoreDisplayFromBackup(&backup)) {
      restored++;
      resetBackup(&backup);
      continue;
    }

    display_backups[remaining++] = backup;
  }

  display_backup_count = remaining;
  if (display_backup_count == 0) {
    clearBackups();
  }

  return restored;
}

uint8_t
ExternalDisplayGammaCopyStateView(size_t index,
                                  ExternalDisplayGammaBackupView *backup_out) {
  if (backup_out == NULL || index >= display_backup_count) {
    return 0;
  }

  DisplayBackup *backup = &display_backups[index];
  backup_out->display_id = backup->display_id;
  backup_out->brightness = backup->brightness;
  backup_out->contrast = backup->contrast;
  backup_out->has_brightness = backup->has_brightness ? 1 : 0;
  backup_out->has_contrast = backup->has_contrast ? 1 : 0;
  backup_out->gamma_sample_count = backup->gamma_sample_count;
  backup_out->gamma_red = backup->gamma_red;
  backup_out->gamma_green = backup->gamma_green;
  backup_out->gamma_blue = backup->gamma_blue;
  return 1;
}

static DisplayBackup
backupFromView(const ExternalDisplayGammaBackupView *view) {
  DisplayBackup backup = {.display_id = 0,
                          .brightness = 0,
                          .contrast = 0,
                          .has_brightness = false,
                          .has_contrast = false,
                          .gamma_sample_count = 0,
                          .gamma_red = NULL,
                          .gamma_green = NULL,
                          .gamma_blue = NULL};
  if (view == NULL) {
    return backup;
  }

  backup.display_id = view->display_id;
  backup.brightness = view->brightness;
  backup.contrast = view->contrast;
  backup.has_brightness = (view->has_brightness != 0);
  backup.has_contrast = (view->has_contrast != 0);
  if (view->gamma_sample_count > 0 && view->gamma_red != NULL &&
      view->gamma_green != NULL && view->gamma_blue != NULL) {
    backup.gamma_sample_count = view->gamma_sample_count;
    backup.gamma_red = (float *)view->gamma_red;
    backup.gamma_green = (float *)view->gamma_green;
    backup.gamma_blue = (float *)view->gamma_blue;
  }

  return backup;
}

size_t ExternalDisplayGammaRestoreFromState(
    const ExternalDisplayGammaBackupView *backups, size_t count) {
  if (backups == NULL && count > 0) {
    clearBackups();
    return 0;
  }

  size_t restored = 0;
  for (size_t i = 0; i < count; i++) {
    DisplayBackup backup = backupFromView(&backups[i]);
    if (backup.display_id != 0 && restoreDisplayFromBackup(&backup)) {
      restored++;
    }
  }

  clearBackups();
  return restored;
}
