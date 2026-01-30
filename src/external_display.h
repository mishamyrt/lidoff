//
//  external_display.h
//  lidoff - external display darkening
//

#ifndef EXTERNAL_DISPLAY_H
#define EXTERNAL_DISPLAY_H

#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>

typedef struct {
    BOOL ok;
    BOOL alreadyDisabled;
    size_t totalExternal;
    size_t disabled;
    size_t failed;
} ExternalDisplayDisableResult;

typedef struct {
    BOOL ok;
    BOOL hadBackups;
    size_t restored;
} ExternalDisplayRestoreResult;

ExternalDisplayDisableResult ExternalDisplaysDisable(void);
ExternalDisplayRestoreResult ExternalDisplaysRestore(void);
ExternalDisplayRestoreResult ExternalDisplaysRestoreFromState(NSDictionary *state);
NSDictionary *ExternalDisplaysCopyState(void);
BOOL ExternalDisplaysAreDisabled(void);

#endif /* EXTERNAL_DISPLAY_H */
