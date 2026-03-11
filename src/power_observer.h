//
//  power_observer.h
//  lidoff - IOKit system power notification observer
//

#ifndef POWER_OBSERVER_H
#define POWER_OBSERVER_H

#import <Foundation/Foundation.h>

typedef void (^PowerObserverWillSleepHandler)(void);
typedef void (^PowerObserverDidWakeHandler)(void);

void PowerObserverStart(PowerObserverWillSleepHandler willSleep,
                        PowerObserverDidWakeHandler didWake);

#endif /* POWER_OBSERVER_H */
