#ifndef LOGGING_H
#define LOGGING_H

#import <Foundation/Foundation.h>

void LogSetVerbose(BOOL enabled);
void LogInfo(NSString *format, ...) NS_FORMAT_FUNCTION(1, 2);
void LogError(NSString *format, ...) NS_FORMAT_FUNCTION(1, 2);
void LogDebug(NSString *format, ...) NS_FORMAT_FUNCTION(1, 2);

#endif /* LOGGING_H */
