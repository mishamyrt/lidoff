#ifndef LID_SENSOR_H
#define LID_SENSOR_H

#include <stdbool.h>

bool LidSensorInit(void);
void LidSensorClose(void);
int LidSensorGetAngle(void);

#endif
