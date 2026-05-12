/* Brightness */

unsafe extern "C" {
    fn BrightnessGet() -> f32;
    fn BrightnessSet(brightness: f32) -> u8;
}

/// Returns the current internal display brightness level.
pub(crate) fn brightness_get() -> f32 {
    unsafe { BrightnessGet() }
}

/// Sets the internal display brightness level.
pub(crate) fn brightness_set(brightness: f32) -> bool {
    unsafe { BrightnessSet(brightness) != 0 }
}

/* Displays */

unsafe extern "C" {
    fn DisplaysListOnline(display_ids: *mut u32, capacity: usize, count_out: *mut usize)
    -> u8;
    fn DisplayIsBuiltin(display_id: u32) -> u8;
}

const MAX_DISPLAY_COUNT: usize = 32;

/// Returns a list of online display IDs.
pub(crate) fn online_displays() -> Option<Vec<u32>> {
    let mut displays = vec![0_u32; MAX_DISPLAY_COUNT];
    let mut count = 0_usize;
    let ok = unsafe {
        c_bool(DisplaysListOnline(displays.as_mut_ptr(), displays.len(), &raw mut count))
    };
    if !ok || count > displays.len() {
        return None;
    }

    displays.truncate(count);
    Some(displays)
}

/// Returns whether the display is built-in (i.e. not an external monitor).
pub(crate) fn is_builtin(display_id: u32) -> bool {
    unsafe { c_bool(DisplayIsBuiltin(display_id)) }
}

/* Skylight */

unsafe extern "C" {
    fn SkylightPrepare(display_count: usize) -> u8;
    fn SkylightFinalize();
    fn SkylightClearBackups();
    fn SkylightCaptureDisplay(display_id: u32) -> u8;
    fn SkylightDisableDisplay(display_id: u32) -> u8;
    fn SkylightBackupCount() -> usize;
    fn SkylightCopyState(display_ids: *mut u32, capacity: usize) -> usize;
    fn SkylightRestoreFromState(display_ids: *const u32, count: usize) -> usize;
}

/// Prepares the Skylight session with the given display count.
pub(crate) fn prepare_skylight(display_count: usize) -> bool {
    unsafe { c_bool(SkylightPrepare(display_count)) }
}

/// Finalizes the Skylight session.
pub(crate) fn finalize_skylight() {
    unsafe { SkylightFinalize() }
}

/// Clears the Skylight backups.
pub(crate) fn clear_skylight_backups() {
    unsafe { SkylightClearBackups() }
}

/// Captures the Skylight display with the given ID.
/// Returns `true` if the display was captured successfully.
pub(crate) fn capture_skylight_display(display_id: u32) -> bool {
    unsafe { c_bool(SkylightCaptureDisplay(display_id)) }
}

/// Disables the Skylight display with the given ID.
/// Returns `true` if the display was disabled successfully.
pub(crate) fn disable_skylight_display(display_id: u32) -> bool {
    unsafe { c_bool(SkylightDisableDisplay(display_id)) }
}

/// Copies the Skylight state into a buffer.
/// Returns the number of display IDs copied.
pub(crate) fn copy_skylight_state() -> Option<Vec<u32>> {
    let count = unsafe { SkylightBackupCount() };
    if count == 0 {
        return Some(Vec::new());
    }

    let mut display_ids = vec![0_u32; count];
    let copied = unsafe { SkylightCopyState(display_ids.as_mut_ptr(), count) };
    (copied == count).then_some(display_ids)
}

/// Restores the Skylight state from the given display IDs.
/// Returns the number of displays that were restored.
pub(crate) fn restore_skylight_state(display_ids: &[u32]) -> usize {
    unsafe { SkylightRestoreFromState(display_ids.as_ptr(), display_ids.len()) }
}

fn c_bool(value: u8) -> bool {
    value != 0
}
