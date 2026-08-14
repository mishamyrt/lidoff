#ifndef LIDOFF_CURSOR_H
#define LIDOFF_CURSOR_H

#include <stdint.h>

uint8_t CursorLock(void);
uint8_t CursorUnlock(void);
uint8_t CursorIsLocked(void);

#endif
