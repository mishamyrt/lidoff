pub const LID_ANGLE_ERROR: i32 = lidoff_lid::LID_ANGLE_ERROR;

pub fn lid_sensor_init() -> bool {
    lidoff_lid::init()
}

pub fn lid_sensor_close() {
    lidoff_lid::close();
}

pub fn lid_sensor_get_angle() -> i32 {
    lidoff_lid::get_angle()
}

pub fn brightness_get() -> f32 {
    lidoff_display::brightness_get()
}

pub fn brightness_set(brightness: f32) -> bool {
    lidoff_display::brightness_set(brightness)
}
