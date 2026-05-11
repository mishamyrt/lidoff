use std::ffi::c_void;
use std::thread;

use crate::lid_sensor;
use crate::logging;

pub const LID_ANGLE_ERROR: i32 = lid_sensor::LID_ANGLE_ERROR;

unsafe extern "C" {
    fn BrightnessGet() -> f32;
    fn BrightnessSet(brightness: f32) -> u8;

    fn CaffeinateStart() -> u8;
    fn CaffeinateStop() -> u8;

    fn PowerObserverRunLoop(
        will_sleep: extern "C" fn(*mut c_void),
        did_wake: extern "C" fn(*mut c_void),
        context: *mut c_void,
    ) -> u8;
}

pub fn lid_sensor_init() -> bool {
    lid_sensor::init()
}

pub fn lid_sensor_close() {
    lid_sensor::close();
}

pub fn lid_sensor_get_angle() -> i32 {
    lid_sensor::get_angle()
}

pub fn brightness_get() -> f32 {
    unsafe { BrightnessGet() }
}

pub fn brightness_set(brightness: f32) -> bool {
    unsafe { c_bool(BrightnessSet(brightness)) }
}

pub fn caffeinate_start() -> bool {
    unsafe { c_bool(CaffeinateStart()) }
}

pub fn caffeinate_stop() -> bool {
    unsafe { c_bool(CaffeinateStop()) }
}

pub fn power_observer_start(
    will_sleep: extern "C" fn(*mut c_void),
    did_wake: extern "C" fn(*mut c_void),
    context: *mut c_void,
) {
    let context = context as usize;
    let spawn_result = thread::Builder::new()
        .name("lidoff.power".to_owned())
        .spawn(move || unsafe {
            if !c_bool(PowerObserverRunLoop(
                will_sleep,
                did_wake,
                context as *mut c_void,
            )) {
                logging::error("power observer failed or exited unexpectedly");
            }
        });

    if let Err(error) = spawn_result {
        logging::error(format!("failed to start power observer thread: {error}"));
    }
}

fn c_bool(value: u8) -> bool {
    value != 0
}
