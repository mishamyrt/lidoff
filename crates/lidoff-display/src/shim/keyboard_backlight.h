#ifndef LIDOFF_KEYBOARD_BACKLIGHT_H
#define LIDOFF_KEYBOARD_BACKLIGHT_H

#include <stdint.h>

float KeyboardBacklightGet(void);
uint8_t KeyboardBacklightSet(float brightness);

#endif /* LIDOFF_KEYBOARD_BACKLIGHT_H */
