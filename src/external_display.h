//
//  external_display.h
//  lidoff - external display darkening
//

#ifndef EXTERNAL_DISPLAY_H
#define EXTERNAL_DISPLAY_H

#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>

BOOL ExternalDisplaysDisable(void);
BOOL ExternalDisplaysRestore(void);
BOOL ExternalDisplaysAreDisabled(void);

// Internal helpers shared across external display methods.
BOOL ExternalDisplaySkylightPrepare(size_t displayCount);
void ExternalDisplaySkylightFinalize(void);
void ExternalDisplaySkylightClearBackups(void);
BOOL ExternalDisplaySkylightDisableDisplay(CGDirectDisplayID displayID);
void ExternalDisplaySkylightRestoreAll(void);
BOOL ExternalDisplaySkylightHasBackups(void);

BOOL ExternalDisplayMirroringPrepare(size_t displayCount);
void ExternalDisplayMirroringFinalize(void);
void ExternalDisplayMirroringClearBackups(void);
BOOL ExternalDisplayMirroringDisableDisplay(CGDirectDisplayID displayID);
void ExternalDisplayMirroringRestoreAll(void);
BOOL ExternalDisplayMirroringHasBackups(void);

BOOL ExternalDisplayGammaPrepare(size_t displayCount);
void ExternalDisplayGammaFinalize(void);
void ExternalDisplayGammaClearBackups(void);
BOOL ExternalDisplayGammaDisableDisplay(CGDirectDisplayID displayID);
void ExternalDisplayGammaRestoreAll(void);
BOOL ExternalDisplayGammaHasBackups(void);

#endif /* EXTERNAL_DISPLAY_H */
