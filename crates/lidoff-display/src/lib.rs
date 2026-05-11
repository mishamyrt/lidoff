mod external_display;

pub use external_display::{
    ExternalDisplayDisableResult, ExternalDisplayRestoreResult, ExternalDisplayState, GammaBackup,
    are_disabled, copy_state, disable, restore, restore_from_state,
};

unsafe extern "C" {
    fn BrightnessGet() -> f32;
    fn BrightnessSet(brightness: f32) -> u8;
}

pub fn brightness_get() -> f32 {
    unsafe { BrightnessGet() }
}

pub fn brightness_set(brightness: f32) -> bool {
    unsafe { c_bool(BrightnessSet(brightness)) }
}

fn c_bool(value: u8) -> bool {
    value != 0
}
