//
//  brightness.h
//  lidoff - display brightness control
//

#ifndef BRIGHTNESS_H
#define BRIGHTNESS_H

#import <Foundation/Foundation.h>

float BrightnessGet(void);
BOOL BrightnessSet(float brightness);
BOOL BrightnessIsDisplayAvailable(void);

#endif /* BRIGHTNESS_H */
