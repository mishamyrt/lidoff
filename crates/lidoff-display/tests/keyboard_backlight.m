#import <Foundation/Foundation.h>
#include <assert.h>

#include "../src/shim/keyboard_backlight.m"

static unsigned live_temporaries = 0;
static bool fail_client = false;

@interface BacklightTemporary : NSObject
@end

@implementation BacklightTemporary
- (id)init {
    self = [super init];
    if (self) {
        live_temporaries++;
    }
    return self;
}
- (void)dealloc {
    live_temporaries--;
    [super dealloc];
}
@end

@interface KeyboardBrightnessClient : NSObject
@end

@implementation KeyboardBrightnessClient
- (BOOL)enableAutoBrightness:(BOOL)enabled forKeyboard:(unsigned long long)keyboard_id {
    (void)enabled;
    (void)keyboard_id;
    assert(!"brightness writes must preserve automatic brightness");
    return NO;
}
- (id)init {
    self = [super init];
    [[[BacklightTemporary alloc] init] autorelease];
    if (fail_client) {
        [self release];
        return nil;
    }
    return self;
}
- (NSArray *)copyKeyboardBacklightIDs {
    return [[NSArray alloc] initWithObjects:@1, nil];
}
- (float)brightnessForKeyboard:(unsigned long long)keyboard_id {
    assert(keyboard_id == 1);
    [[[BacklightTemporary alloc] init] autorelease];
    return 0.625f;
}
- (BOOL)setBrightness:(float)brightness forKeyboard:(unsigned long long)keyboard_id {
    assert(keyboard_id == 1);
    assert(brightness == 0.0f || brightness == 0.625f);
    [[[BacklightTemporary alloc] init] autorelease];
    return YES;
}
@end

int main(void) {
    // Use the fake client without loading CoreBrightness or changing hardware state.
    corebrightness_loaded = true;
    corebrightness_handle = (void *)1;

    @autoreleasepool {
        for (int i = 0; i < 100; i++) {
            assert(KeyboardBacklightGet() == 0.625f);
            assert(live_temporaries == 0);
            assert(KeyboardBacklightSet(0.0f) == 1);
            assert(live_temporaries == 0);
            assert(KeyboardBacklightSet(0.625f) == 1);
            assert(live_temporaries == 0);
        }

        fail_client = true;
        assert(KeyboardBacklightGet() == -1.0f);
        assert(live_temporaries == 0);
        assert(KeyboardBacklightSet(0.625f) == 0);
        assert(live_temporaries == 0);
    }
    return 0;
}
