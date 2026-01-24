//
//  lid_sensor.h
//  lidoff - MacBook lid angle HID sensor
//

#ifndef LID_SENSOR_H
#define LID_SENSOR_H

#import <Foundation/Foundation.h>
#import <IOKit/hid/IOHIDManager.h>

#define LID_SENSOR_VID        0x05AC
#define LID_SENSOR_PID        0x8104
#define LID_SENSOR_USAGE_PAGE 0x0020
#define LID_SENSOR_USAGE      0x008A

#define LID_ANGLE_ERROR       -1

BOOL LidSensorInit(void);
void LidSensorClose(void);
int LidSensorGetAngle(void);
BOOL LidSensorIsAvailable(void);

#endif /* LID_SENSOR_H */
