use std::ffi::{c_int, c_uchar};
use thiserror::Error;

/// Represents errors that can occur when interacting with the lid sensor.
#[derive(Error, Debug)]
pub enum SensorError {
    #[error("lid sensor initialization failed")]
    InitFailed,

    #[error("lid sensor angle reading failed")]
    ReadFailed,
}

unsafe extern "C" {
    fn LidSensorInit() -> c_uchar;
    fn LidSensorClose();
    fn LidSensorGetAngle() -> c_int;
}

const LID_ANGLE_ERROR: i32 = -1;

/// Initializes the lid sensor.
///
/// Returns an error if the sensor could not be initialized.
pub fn init() -> Result<(), SensorError> {
    if unsafe { LidSensorInit() == 0 } { Err(SensorError::InitFailed) } else { Ok(()) }
}

/// Closes connection with the lid sensor.
pub fn close() {
    unsafe {
        LidSensorClose();
    }
}

/// Returns the current lid angle as an integer.
///
/// Returns an error if the angle could not be read.
pub fn get_angle() -> Result<i32, SensorError> {
    unsafe {
        let angle = LidSensorGetAngle();
        if angle == LID_ANGLE_ERROR { Err(SensorError::ReadFailed) } else { Ok(angle) }
    }
}
