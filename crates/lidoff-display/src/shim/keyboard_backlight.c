#include "keyboard_backlight.h"

#include <dlfcn.h>
#include <objc/message.h>
#include <objc/objc.h>
#include <objc/runtime.h>
#include <stdbool.h>
#include <stddef.h>

#ifndef NSUInteger
typedef unsigned long NSUInteger;
#endif

#define COREBRIGHTNESS_PATH \
    "/System/Library/PrivateFrameworks/CoreBrightness.framework/CoreBrightness"

static void *corebrightness_handle = NULL;
static bool corebrightness_loaded = false;

static bool loadCoreBrightness(void) {
    if (corebrightness_loaded) {
        return corebrightness_handle != NULL;
    }

    corebrightness_loaded = true;
    corebrightness_handle = dlopen(COREBRIGHTNESS_PATH, RTLD_NOW | RTLD_GLOBAL);
    return corebrightness_handle != NULL;
}

static id newKeyboardBrightnessClient(void) {
    if (!loadCoreBrightness()) {
        return nil;
    }

    Class keyboard_class = objc_getClass("KeyboardBrightnessClient");
    if (keyboard_class == Nil) {
        return nil;
    }

    id client = ((id (*)(id, SEL))objc_msgSend)((id)keyboard_class, sel_registerName("alloc"));
    return ((id (*)(id, SEL))objc_msgSend)(client, sel_registerName("init"));
}

static void releaseObject(id object) {
    if (object != nil) {
        ((void (*)(id, SEL))objc_msgSend)(object, sel_registerName("release"));
    }
}

static bool copyFirstKeyboardId(id client, unsigned long long *keyboard_id) {
    SEL ids_selector = sel_registerName("copyKeyboardBacklightIDs");
    if (!class_getInstanceMethod(object_getClass(client), ids_selector)) {
        *keyboard_id = 1;
        return true;
    }

    id ids = ((id (*)(id, SEL))objc_msgSend)(client, ids_selector);
    if (ids == nil) {
        *keyboard_id = 1;
        return true;
    }

    NSUInteger count = ((NSUInteger (*)(id, SEL))objc_msgSend)(ids, sel_registerName("count"));
    if (count == 0) {
        releaseObject(ids);
        *keyboard_id = 1;
        return true;
    }

    id number = ((id (*)(id, SEL, NSUInteger))objc_msgSend)(
        ids, sel_registerName("objectAtIndex:"), 0);
    *keyboard_id = ((unsigned long long (*)(id, SEL))objc_msgSend)(
        number, sel_registerName("unsignedLongLongValue"));
    releaseObject(ids);
    return true;
}

float KeyboardBacklightGet(void) {
    id client = newKeyboardBrightnessClient();
    if (client == nil) {
        return -1.0f;
    }

    unsigned long long keyboard_id = 1;
    if (!copyFirstKeyboardId(client, &keyboard_id)) {
        releaseObject(client);
        return -1.0f;
    }

    SEL brightness_selector = sel_registerName("brightnessForKeyboard:");
    if (!class_getInstanceMethod(object_getClass(client), brightness_selector)) {
        releaseObject(client);
        return -1.0f;
    }

    float brightness = ((float (*)(id, SEL, unsigned long long))objc_msgSend)(
        client, brightness_selector, keyboard_id);
    releaseObject(client);
    return brightness;
}

static bool setKeyboardBrightness(id client,
                                  unsigned long long keyboard_id,
                                  float brightness) {
    SEL auto_selector = sel_registerName("enableAutoBrightness:forKeyboard:");
    if (class_getInstanceMethod(object_getClass(client), auto_selector)) {
        ((BOOL (*)(id, SEL, BOOL, unsigned long long))objc_msgSend)(client, auto_selector,
                                                                    (BOOL)0, keyboard_id);
    }

    SEL fade_selector = sel_registerName("setBrightness:fadeSpeed:commit:forKeyboard:");
    if (class_getInstanceMethod(object_getClass(client), fade_selector)) {
        BOOL ok = ((BOOL (*)(id, SEL, float, int, BOOL, unsigned long long))objc_msgSend)(
            client, fade_selector, brightness, 350, (BOOL)1, keyboard_id);
        return ok ? true : false;
    }

    SEL simple_selector = sel_registerName("setBrightness:forKeyboard:");
    if (class_getInstanceMethod(object_getClass(client), simple_selector)) {
        BOOL ok = ((BOOL (*)(id, SEL, float, unsigned long long))objc_msgSend)(
            client, simple_selector, brightness, keyboard_id);
        return ok ? true : false;
    }

    return false;
}

uint8_t KeyboardBacklightSet(float brightness) {
    if (brightness < 0.0f) {
        brightness = 0.0f;
    }
    if (brightness > 1.0f) {
        brightness = 1.0f;
    }

    id client = newKeyboardBrightnessClient();
    if (client == nil) {
        return 0;
    }

    bool any_ok = false;
    SEL ids_selector = sel_registerName("copyKeyboardBacklightIDs");
    if (class_getInstanceMethod(object_getClass(client), ids_selector)) {
        id ids = ((id (*)(id, SEL))objc_msgSend)(client, ids_selector);

        if (ids != nil) {
            NSUInteger count =
                ((NSUInteger (*)(id, SEL))objc_msgSend)(ids, sel_registerName("count"));

            for (NSUInteger i = 0; i < count; i++) {
                id number = ((id (*)(id, SEL, NSUInteger))objc_msgSend)(
                    ids, sel_registerName("objectAtIndex:"), i);
                unsigned long long keyboard_id =
                    ((unsigned long long (*)(id, SEL))objc_msgSend)(
                        number, sel_registerName("unsignedLongLongValue"));

                if (setKeyboardBrightness(client, keyboard_id, brightness)) {
                    any_ok = true;
                }
            }

            releaseObject(ids);
        }
    }

    if (!any_ok) {
        any_ok = setKeyboardBrightness(client, 1, brightness);
    }

    releaseObject(client);
    return any_ok ? 1 : 0;
}
