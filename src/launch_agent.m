#import "launch_agent.h"
#import "logging.h"

static NSString *const kLaunchAgentLabel = @"co.myrt.lidoff";
static NSString *const kLaunchAgentPath = @"~/Library/LaunchAgents/co.myrt.lidoff.plist";

static NSString *launchAgentPlistPath(void) {
    return [kLaunchAgentPath stringByExpandingTildeInPath];
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
        kLaunchAgentLabel, realPath, threshold];
}

static BOOL runLaunchctl(NSArray<NSString *> *arguments, NSString **outputOut) {
    NSTask *task = [[NSTask alloc] init];
    task.launchPath = @"/bin/launchctl";
    task.arguments = arguments;
    
    NSPipe *pipe = [NSPipe pipe];
    task.standardOutput = pipe;
    task.standardError = pipe;
    
    @try {
        [task launch];
        [task waitUntilExit];
    } @catch (NSException *exception) {
        LogError(@"launchctl threw exception: %@", exception.reason ?: @"unknown");
        return NO;
    }
    
    NSData *outputData = [[pipe fileHandleForReading] readDataToEndOfFile];
    if (outputOut) {
        *outputOut = outputData.length > 0
            ? [[NSString alloc] initWithData:outputData encoding:NSUTF8StringEncoding]
            : nil;
    }
    
    if (task.terminationStatus != 0) {
        return NO;
    }
    
    return YES;
}

BOOL LaunchAgentInstall(int threshold) {
    NSString *plistPath = launchAgentPlistPath();
    NSString *plistDir = [plistPath stringByDeletingLastPathComponent];
    
    NSFileManager *fm = [NSFileManager defaultManager];
    NSError *error = nil;
    
    if (![fm fileExistsAtPath:plistDir]) {
        if (![fm createDirectoryAtPath:plistDir
           withIntermediateDirectories:YES
                            attributes:nil
                                 error:&error]) {
            LogError(@"failed to create directory %@", plistDir);
            return NO;
        }
    }
    
    NSString *content = generatePlistContent(threshold);
    if (![content writeToFile:plistPath
                   atomically:YES
                     encoding:NSUTF8StringEncoding
                        error:&error]) {
        LogError(@"failed to write %@", plistPath);
        return NO;
    }
    
    NSString *output = nil;
    if (!runLaunchctl(@[@"load", plistPath], &output)) {
        if (output.length > 0) {
            LogError(@"launchctl load failed: %@", output);
        } else {
            LogError(@"launchctl load failed");
        }
        return NO;
    }
    
    LogInfo(@"installed (%@)", plistPath);
    LogInfo(@"threshold: %d°", threshold);
    return YES;
}

BOOL LaunchAgentUninstall(void) {
    NSString *plistPath = launchAgentPlistPath();
    NSFileManager *fm = [NSFileManager defaultManager];
    
    if (![fm fileExistsAtPath:plistPath]) {
        LogInfo(@"not installed");
        return YES;
    }
    
    NSString *output = nil;
    if (!runLaunchctl(@[@"unload", plistPath], &output)) {
        if (output.length > 0) {
            LogError(@"launchctl unload failed: %@", output);
        } else {
            LogError(@"launchctl unload failed");
        }
        return NO;
    }
    
    NSError *error = nil;
    if (![fm removeItemAtPath:plistPath error:&error]) {
        LogError(@"failed to remove %@", plistPath);
        return NO;
    }
    
    LogInfo(@"uninstalled");
    return YES;
}
