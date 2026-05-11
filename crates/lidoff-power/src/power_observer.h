#ifndef POWER_OBSERVER_H
#define POWER_OBSERVER_H

#include <stdint.h>

typedef void (*PowerObserverCallback)(void *context);

uint8_t PowerObserverRunLoop(PowerObserverCallback will_sleep,
                             PowerObserverCallback did_wake, void *context);

uint8_t CaffeinateStart(void);
uint8_t CaffeinateStop(void);

#endif /* POWER_OBSERVER_H */
