//
//  main.m
//  lidoff - MacBook lid angle brightness daemon
//

#import <Foundation/Foundation.h>
#import <signal.h>
#import "lid_sensor.h"
#import "brightness.h"

#define DEFAULT_THRESHOLD   30
#define DEFAULT_INTERVAL    500

#define LAUNCH_AGENT_LABEL  @"co.myrt.lidoff"
#define LAUNCH_AGENT_PATH   @"~/Library/LaunchAgents/co.myrt.lidoff.plist"

static BOOL shouldRun = YES;
static float savedBrightness = -1.0f;
static BOOL brightnessLowered = NO;
static BOOL verbose = NO;

static void signalHandler(int sig) {
    (void)sig;
    shouldRun = NO;
}

static void printUsage(const char *programName) {
    printf("lidoff - MacBook lid angle brightness daemon\n\n");
    printf("Usage:\n");
    printf("  %s [-t threshold] [-i interval]  Run daemon\n", programName);
    printf("  %s --install [-t threshold]      Install as LaunchAgent\n", programName);
    printf("  %s --uninstall                   Remove LaunchAgent\n", programName);
    printf("  %s --help                        Show this help\n\n", programName);
    printf("Options:\n");
    printf("  -t, --threshold <degrees>   Lid angle threshold (default: %d)\n", DEFAULT_THRESHOLD);
    printf("  -i, --interval <ms>         Polling interval in ms (default: %d)\n", DEFAULT_INTERVAL);
    printf("  -v, --verbose               Log current lid angle\n\n");
    printf("Behavior:\n");
    printf("  angle < threshold: save brightness, set to 0\n");
    printf("  angle >= threshold: restore saved brightness\n");
}

static NSString *getExecutablePath(void) {
    return [[NSBundle mainBundle] executablePath] ?: 
           [NSString stringWithUTF8String:getenv("_") ?: "/usr/local/bin/lidoff"];
}

static NSString *generatePlistContent(int threshold) {
    NSString *execPath = getExecutablePath();
    NSString *realPath = [[NSFileManager defaultManager] 
                          destinationOfSymbolicLinkAtPath:execPath error:nil] ?: execPath;
    
    if (![realPath hasPrefix:@"/"]) {
        realPath = [[[NSFileManager defaultManager] currentDirectoryPath] 
                    stringByAppendingPathComponent:realPath];
    }
    
    return [NSString stringWithFormat:
        @"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
        @"<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n"
        @"<plist version=\"1.0\">\n"
        @"<dict>\n"
        @"    <key>Label</key>\n"
        @"    <string>%@</string>\n"
        @"    <key>ProgramArguments</key>\n"
        @"    <array>\n"
        @"        <string>%@</string>\n"
        @"        <string>-t</string>\n"
        @"        <string>%d</string>\n"
        @"    </array>\n"
        @"    <key>RunAtLoad</key>\n"
        @"    <true/>\n"
        @"    <key>KeepAlive</key>\n"
        @"    <true/>\n"
        @"    <key>StandardOutPath</key>\n"
        @"    <string>/tmp/lidoff.log</string>\n"
        @"    <key>StandardErrorPath</key>\n"
        @"    <string>/tmp/lidoff.err</string>\n"
        @"</dict>\n"
        @"</plist>\n",
        LAUNCH_AGENT_LABEL, realPath, threshold];
}

static BOOL installLaunchAgent(int threshold) {
    NSString *plistPath = [LAUNCH_AGENT_PATH stringByExpandingTildeInPath];
    NSString *plistDir = [plistPath stringByDeletingLastPathComponent];
    
    NSFileManager *fm = [NSFileManager defaultManager];
    NSError *error = nil;
    
    if (![fm fileExistsAtPath:plistDir]) {
        if (![fm createDirectoryAtPath:plistDir 
               withIntermediateDirectories:YES 
                                attributes:nil 
                                     error:&error]) {
            fprintf(stderr, "lidoff: failed to create directory %s\n", plistDir.UTF8String);
            return NO;
        }
    }
    
    NSString *content = generatePlistContent(threshold);
    if (![content writeToFile:plistPath 
                   atomically:YES 
                     encoding:NSUTF8StringEncoding 
                        error:&error]) {
        fprintf(stderr, "lidoff: failed to write %s\n", plistPath.UTF8String);
        return NO;
    }
    
    NSTask *task = [[NSTask alloc] init];
    task.launchPath = @"/bin/launchctl";
    task.arguments = @[@"load", plistPath];
    
    @try {
        [task launch];
        [task waitUntilExit];
    } @catch (NSException *e) {
        fprintf(stderr, "lidoff: launchctl error\n");
        return NO;
    }
    
    printf("lidoff: installed (%s)\n", plistPath.UTF8String);
    printf("lidoff: threshold: %d°\n", threshold);
    return YES;
}

static BOOL uninstallLaunchAgent(void) {
    NSString *plistPath = [LAUNCH_AGENT_PATH stringByExpandingTildeInPath];
    NSFileManager *fm = [NSFileManager defaultManager];
    
    if (![fm fileExistsAtPath:plistPath]) {
        printf("lidoff: not installed\n");
        return YES;
    }
    
    NSTask *task = [[NSTask alloc] init];
    task.launchPath = @"/bin/launchctl";
    task.arguments = @[@"unload", plistPath];
    
    @try {
        [task launch];
        [task waitUntilExit];
    } @catch (NSException *e) {
        // ignore
    }
    
    NSError *error = nil;
    if (![fm removeItemAtPath:plistPath error:&error]) {
        fprintf(stderr, "lidoff: failed to remove %s\n", plistPath.UTF8String);
        return NO;
    }
    
    printf("lidoff: uninstalled\n");
    return YES;
}

static void runMonitor(int threshold, int intervalMs) {
    NSTimeInterval interval = intervalMs / 1000.0;
    
    while (shouldRun) {
        @autoreleasepool {
            int angle = LidSensorGetAngle();
            
            if (angle == LID_ANGLE_ERROR) {
                [NSThread sleepForTimeInterval:interval];
                continue;
            }
            
            if (verbose) {
                NSLog(@"lidoff: angle %d°", angle);
            }
            
            if (angle < threshold) {
                if (!brightnessLowered) {
                    savedBrightness = BrightnessGet();
                    if (savedBrightness >= 0.0f) {
                        BrightnessSet(0.0f);
                        brightnessLowered = YES;
                    }
                }
            } else {
                if (brightnessLowered && savedBrightness >= 0.0f) {
                    BrightnessSet(savedBrightness);
                    brightnessLowered = NO;
                    savedBrightness = -1.0f;
                }
            }
            
            [NSThread sleepForTimeInterval:interval];
        }
    }
    
    if (brightnessLowered && savedBrightness >= 0.0f) {
        BrightnessSet(savedBrightness);
    }
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        int threshold = DEFAULT_THRESHOLD;
        int interval = DEFAULT_INTERVAL;
        BOOL doInstall = NO;
        BOOL doUninstall = NO;
        
        for (int i = 1; i < argc; i++) {
            const char *arg = argv[i];
            
            if (strcmp(arg, "--help") == 0 || strcmp(arg, "-h") == 0) {
                printUsage(argv[0]);
                return 0;
            }
            else if (strcmp(arg, "--install") == 0) {
                doInstall = YES;
            }
            else if (strcmp(arg, "--uninstall") == 0) {
                doUninstall = YES;
            }
            else if ((strcmp(arg, "-t") == 0 || strcmp(arg, "--threshold") == 0) && i + 1 < argc) {
                threshold = atoi(argv[++i]);
                if (threshold < 0 || threshold > 180) {
                    fprintf(stderr, "lidoff: invalid threshold: %d (0-180)\n", threshold);
                    return 1;
                }
            }
            else if ((strcmp(arg, "-i") == 0 || strcmp(arg, "--interval") == 0) && i + 1 < argc) {
                interval = atoi(argv[++i]);
                if (interval < 100 || interval > 10000) {
                    fprintf(stderr, "lidoff: invalid interval: %d (100-10000)\n", interval);
                    return 1;
                }
            }
            else if (strcmp(arg, "-v") == 0 || strcmp(arg, "--verbose") == 0) {
                verbose = YES;
            }
            else {
                fprintf(stderr, "lidoff: unknown option: %s\n", arg);
                printUsage(argv[0]);
                return 1;
            }
        }
        
        if (doUninstall) {
            return uninstallLaunchAgent() ? 0 : 1;
        }
        
        if (doInstall) {
            return installLaunchAgent(threshold) ? 0 : 1;
        }
        
        signal(SIGINT, signalHandler);
        signal(SIGTERM, signalHandler);
        
        if (!LidSensorInit()) {
            fprintf(stderr, "lidoff: failed to initialize lid sensor\n");
            return 1;
        }
        
        runMonitor(threshold, interval);
        
        LidSensorClose();
        
        return 0;
    }
}
