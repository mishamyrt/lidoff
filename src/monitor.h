#ifndef MONITOR_H
#define MONITOR_H

#import <Foundation/Foundation.h>
#import <signal.h>

extern const int MonitorDefaultThreshold;
extern const int MonitorDefaultIntervalMs;
extern const int MonitorFullCloseAngle;
extern const int MonitorPartialStabilitySamples;
extern const NSTimeInterval MonitorPostCloseGraceSeconds;
extern const NSTimeInterval MonitorPostWakeGraceSeconds;

typedef struct {
    int threshold;
    int intervalMs;
} MonitorConfig;

void MonitorRun(const MonitorConfig *config, volatile sig_atomic_t *shouldRunFlag);

#endif /* MONITOR_H */
