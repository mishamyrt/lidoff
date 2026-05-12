use std::{
    ffi::{c_int, c_uchar},
    marker::PhantomData,
    rc::Rc,
};
use thiserror::Error;

/// Represents errors that can occur when interacting with the lid sensor.
#[derive(Error, Debug)]
pub enum SensorError {
    #[error("lid sensor initialization failed")]
    InitFailed,

    #[error("lid sensor angle reading failed")]
    ReadFailed,

    #[error("lid sensor angle out of range")]
    AngleOutOfRange,
}

unsafe extern "C" {
    fn LidSensorInit() -> c_uchar;
    fn LidSensorClose();
    fn LidSensorGetAngle() -> c_int;
}

const LID_ANGLE_ERROR: i32 = -1;

#[derive(Debug)]
pub struct LidSensor {
    _not_thread_safe: PhantomData<Rc<()>>,
}

impl LidSensor {
    /// Initializes the lid sensor.
    ///
    /// Returns an error if the sensor could not be initialized.
    pub fn new() -> Result<Self, SensorError> {
        if unsafe { LidSensorInit() == 0 } {
            Err(SensorError::InitFailed)
        } else {
            Ok(Self { _not_thread_safe: PhantomData })
        }
    }

    /// Returns the current lid angle as an integer.
    ///
    /// Returns an error if the angle could not be read.
    pub fn get_angle(&mut self) -> Result<u32, SensorError> {
        unsafe {
            let angle = LidSensorGetAngle();
            if angle == LID_ANGLE_ERROR {
                Err(SensorError::ReadFailed)
            } else {
                let uangle = angle.try_into().map_err(|_| SensorError::AngleOutOfRange)?;
                Ok(uangle)
            }
        }
    }
}

impl Drop for LidSensor {
    fn drop(&mut self) {
        unsafe {
            LidSensorClose();
        }
    }
}
