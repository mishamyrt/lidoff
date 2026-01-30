#ifndef EXTERNAL_DISPLAY_BACKEND_H
#define EXTERNAL_DISPLAY_BACKEND_H

#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>

typedef struct {
    const char *name;
    BOOL (*prepare)(size_t displayCount);
    void (*finalize)(void);
    void (*clearBackups)(void);
    BOOL (*disableDisplay)(CGDirectDisplayID displayID);
    void (*restoreAll)(void);
    BOOL (*hasBackups)(void);
    NSDictionary *(*copyState)(void);
    BOOL (*restoreFromState)(NSDictionary *state, size_t *restoredCount);
} ExternalDisplayBackend;

const ExternalDisplayBackend *ExternalDisplayBackendSkylight(void);
const ExternalDisplayBackend *ExternalDisplayBackendMirroring(void);
const ExternalDisplayBackend *ExternalDisplayBackendGamma(void);

#endif /* EXTERNAL_DISPLAY_BACKEND_H */
