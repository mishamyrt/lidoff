#include <CoreGraphics/CoreGraphics.h>
#include <assert.h>
#include <stdbool.h>

static int config_token;
static unsigned commits = 0;
static unsigned cancels = 0;
static CGError commit_result = kCGErrorSuccess;
static Boolean expected_enabled = 0;
static bool contains_stale_display = false;

static CGError beginConfiguration(CGDisplayConfigRef *config) {
    contains_stale_display = false;
    *config = (CGDisplayConfigRef)&config_token;
    return kCGErrorSuccess;
}

static CGError completeConfiguration(CGDisplayConfigRef config, CGConfigureOption option) {
    assert(config == (CGDisplayConfigRef)&config_token);
    assert(option == kCGConfigureForSession);
    commits++;
    return contains_stale_display ? kCGErrorFailure : commit_result;
}

static CGError cancelConfiguration(CGDisplayConfigRef config) {
    assert(config == (CGDisplayConfigRef)&config_token);
    cancels++;
    return kCGErrorSuccess;
}

static CGError configureDisplay(CGDisplayConfigRef config,
                                CGDirectDisplayID display_id,
                                Boolean enabled) {
    assert(config == (CGDisplayConfigRef)&config_token);
    assert(enabled == expected_enabled);
    if (display_id == 6) {
        contains_stale_display = true;
    }
    return display_id == 2 ? kCGErrorFailure : kCGErrorSuccess;
}

// Compile the real shim against fake display configuration calls, without touching screens.
#define CGBeginDisplayConfiguration beginConfiguration
#define CGCompleteDisplayConfiguration completeConfiguration
#define CGCancelDisplayConfiguration cancelConfiguration
#include "../src/shim/skylight.c"

int main(void) {
    skylight_loaded = true;
    skylight_available = true;
    SLSConfigureDisplayEnabledPtr = configureDisplay;

    assert(SkylightDisableDisplay(1) == 1);
    assert(SkylightDisableDisplay(2) == 0);
    assert(SkylightDisableDisplay(3) == 1);
    assert(commits == 0);
    assert(SkylightFinalize() == 1);
    assert(commits == 1);
    assert(SkylightFinalize() == 1);
    assert(commits == 1);

    expected_enabled = 1;
    const CGDirectDisplayID display_ids[] = {1, 2, 3};
    assert(SkylightRestoreFromState(display_ids, 3) == 2);
    assert(commits == 3);
    assert(cancels == 1);

    commit_result = kCGErrorFailure;
    assert(SkylightRestoreFromState(display_ids, 3) == 0);
    assert(commits == 5);
    assert(cancels == 4);

    commit_result = kCGErrorSuccess;
    assert(SkylightRestoreFromState(display_ids, 3) == 2);
    assert(commits == 7);

    assert(SkylightRestoreFromState(&display_ids[1], 1) == 0);
    assert(cancels == 6);
    assert(SkylightRestoreFromState(NULL, 0) == 0);
    assert(SkylightRestoreFromState(NULL, 1) == 0);
    assert(commits == 7);

    const CGDirectDisplayID mixed_ids[] = {6, 1, 2, 3};
    assert(SkylightRestoreFromState(mixed_ids, 4) == 2);
    assert(commits == 10);
    assert(cancels == 8);
    return 0;
}
