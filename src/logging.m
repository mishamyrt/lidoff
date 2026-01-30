#import "logging.h"
#import <stdarg.h>
#include <stdio.h>

static BOOL verboseEnabled = NO;

void LogSetVerbose(BOOL enabled) {
    verboseEnabled = enabled;
}

static void logMessage(FILE *stream, const char *level, NSString *format, va_list args) {
    if (!format || !stream || !level) {
        return;
    }
    
    NSString *message = [[NSString alloc] initWithFormat:format arguments:args];
    if (!message) {
        return;
    }
    
    fprintf(stream, "lidoff[%s]: %s\n", level, message.UTF8String);
    fflush(stream);
}

void LogInfo(NSString *format, ...) {
    va_list args;
    va_start(args, format);
    logMessage(stdout, "info", format, args);
    va_end(args);
}

void LogError(NSString *format, ...) {
    va_list args;
    va_start(args, format);
    logMessage(stderr, "error", format, args);
    va_end(args);
}

void LogDebug(NSString *format, ...) {
    if (!verboseEnabled) {
        return;
    }
    
    va_list args;
    va_start(args, format);
    logMessage(stdout, "debug", format, args);
    va_end(args);
}
