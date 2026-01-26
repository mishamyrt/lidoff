//
//  caffeinate.m
//  lidoff - caffeinate session management
//

#import "caffeinate.h"
#import <IOKit/pwr_mgt/IOPMLib.h>

static IOPMAssertionID assertionID = 0;
static BOOL caffeinateActive = NO;

BOOL CaffeinateStart(void) {
    if (caffeinateActive) {
        return YES;
    }
    
    CFStringRef reason = CFSTR("lidoff: lid partially closed");
    IOReturn result = IOPMAssertionCreateWithName(
        kIOPMAssertionTypePreventUserIdleDisplaySleep,
        kIOPMAssertionLevelOn,
        reason,
        &assertionID
    );
    
    if (result == kIOReturnSuccess) {
        caffeinateActive = YES;
        return YES;
    }
    
    return NO;
}

BOOL CaffeinateStop(void) {
    if (!caffeinateActive) {
        return YES;
    }
    
    IOReturn result = IOPMAssertionRelease(assertionID);
    
    if (result == kIOReturnSuccess) {
        assertionID = 0;
        caffeinateActive = NO;
        return YES;
    }
    
    return NO;
}

BOOL CaffeinateIsActive(void) {
    return caffeinateActive;
}
