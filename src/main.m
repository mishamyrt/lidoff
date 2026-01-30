//
//  main.m
//  lidoff - MacBook lid angle brightness daemon
//

#import <Foundation/Foundation.h>
#import <signal.h>
#import "lid_sensor.h"
#import "launch_agent.h"
#import "monitor.h"
#import "logging.h"

#define VERSION_STRING [NSString stringWithFormat:@"%s", VERSION]

static volatile sig_atomic_t shouldRun = 1;

static void signalHandler(int sig) {
    (void)sig;
    shouldRun = 0;
}

static void printUsage(const char *programName) {
    printf("lidoff - MacBook lid angle brightness daemon\n\n");
    printf("Usage:\n");
    printf("  %s [-t threshold] [-i interval]  Run daemon\n", programName);
    printf("  %s --enable [-t threshold]      Install as LaunchAgent\n", programName);
    printf("  %s --disable                   Remove LaunchAgent\n", programName);
    printf("  %s --help                        Show this help\n\n", programName);
    printf("  %s --version                     Show version\n\n", programName);
    printf("Options:\n");
    printf("  -t, --threshold <degrees>   Lid angle threshold (default: %d)\n", MonitorDefaultThreshold);
    printf("  -i, --interval <ms>         Polling interval in ms (default: %d)\n", MonitorDefaultIntervalMs);
    printf("  -v, --verbose               Log current lid angle\n\n");
    printf("Behavior:\n");
    printf("  angle < %d: fully closed, restore brightness and end caffeinate\n", MonitorFullCloseAngle);
    printf("  angle < threshold: save brightness, set to 0, start caffeinate\n");
    printf("  angle >= threshold: restore saved brightness, end caffeinate\n");
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        int threshold = MonitorDefaultThreshold;
        int interval = MonitorDefaultIntervalMs;
        BOOL doInstall = NO;
        BOOL doUninstall = NO;
        BOOL verbose = NO;
        
        for (int i = 1; i < argc; i++) {
            const char *arg = argv[i];
            
            if (strcmp(arg, "--help") == 0 || strcmp(arg, "-h") == 0) {
                printUsage(argv[0]);
                return 0;
            }
            if (strcmp(arg, "--version") == 0) {
                printf("%s\n", VERSION_STRING.UTF8String);
                return 0;
            }
            if (strcmp(arg, "--enable") == 0) {
                doInstall = YES;
                continue;
            }
            if (strcmp(arg, "--disable") == 0) {
                doUninstall = YES;
                continue;
            }
            if ((strcmp(arg, "-t") == 0 || strcmp(arg, "--threshold") == 0) && i + 1 < argc) {
                threshold = atoi(argv[++i]);
                if (threshold < 0 || threshold > 180) {
                    fprintf(stderr, "lidoff: invalid threshold: %d (0-180)\n", threshold);
                    return 1;
                }
                continue;
            }
            if ((strcmp(arg, "-i") == 0 || strcmp(arg, "--interval") == 0) && i + 1 < argc) {
                interval = atoi(argv[++i]);
                if (interval < 100 || interval > 10000) {
                    fprintf(stderr, "lidoff: invalid interval: %d (100-10000)\n", interval);
                    return 1;
                }
                continue;
            }
            if (strcmp(arg, "-v") == 0 || strcmp(arg, "--verbose") == 0) {
                verbose = YES;
                continue;
            }
            
            fprintf(stderr, "lidoff: unknown option: %s\n", arg);
            printUsage(argv[0]);
            return 1;
        }
        
        LogSetVerbose(verbose);
        
        if (doUninstall) {
            return LaunchAgentUninstall() ? 0 : 1;
        }
        
        if (doInstall) {
            return LaunchAgentInstall(threshold) ? 0 : 1;
        }
        
        signal(SIGINT, signalHandler);
        signal(SIGTERM, signalHandler);
        signal(SIGHUP, signalHandler);
        signal(SIGQUIT, signalHandler);
        
        if (!LidSensorInit()) {
            LogError(@"failed to initialize lid sensor");
            return 1;
        }
        
        MonitorConfig config = {
            .threshold = threshold,
            .intervalMs = interval
        };
        
        MonitorRun(&config, &shouldRun);
        
        LidSensorClose();
        return 0;
    }
}
