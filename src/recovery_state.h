#ifndef RECOVERY_STATE_H
#define RECOVERY_STATE_H

#import <Foundation/Foundation.h>

typedef struct {
    BOOL pendingBrightnessRestore;
    float savedBrightness;
    BOOL pendingExternalRestore;
} RecoveryState;

NSString *RecoveryStatePath(void);
BOOL RecoveryStateLoad(RecoveryState *stateOut, NSDictionary **externalStateOut);
BOOL RecoveryStateSave(const RecoveryState *state, NSDictionary *externalState);
void RecoveryStateClear(void);

#endif /* RECOVERY_STATE_H */
