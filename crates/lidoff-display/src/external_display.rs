use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

const MAX_DISPLAY_COUNT: usize = 32;

static EXTERNAL_DISPLAYS_DISABLED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Default)]
pub struct ExternalDisplayDisableResult {
    pub ok: bool,
    pub already_disabled: bool,
    pub total_external: usize,
    pub disabled: usize,
    pub failed: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ExternalDisplayRestoreResult {
    pub ok: bool,
    pub had_backups: bool,
    pub restored: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExternalDisplayState {
    pub skylight_display_ids: Vec<u32>,
    pub gamma_backups: Vec<GammaBackup>,
}

impl ExternalDisplayState {
    fn has_backups(&self) -> bool {
        !self.skylight_display_ids.is_empty() || !self.gamma_backups.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GammaBackup {
    pub display_id: u32,
    pub brightness: Option<u16>,
    pub contrast: Option<u16>,
    pub gamma_red: Vec<f32>,
    pub gamma_green: Vec<f32>,
    pub gamma_blue: Vec<f32>,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CExternalDisplayGammaBackupView {
    display_id: u32,
    brightness: u16,
    contrast: u16,
    has_brightness: u8,
    has_contrast: u8,
    gamma_sample_count: u32,
    gamma_red: *const f32,
    gamma_green: *const f32,
    gamma_blue: *const f32,
}

unsafe extern "C" {
    fn ExternalDisplayGetOnlineDisplays(
        display_ids: *mut u32,
        capacity: usize,
        count_out: *mut usize,
    ) -> u8;
    fn ExternalDisplayIsBuiltin(display_id: u32) -> u8;

    fn ExternalDisplaySkylightPrepare(display_count: usize) -> u8;
    fn ExternalDisplaySkylightFinalize();
    fn ExternalDisplaySkylightClearBackups();
    fn ExternalDisplaySkylightDisableDisplay(display_id: u32) -> u8;
    fn ExternalDisplaySkylightRestoreAll() -> usize;
    fn ExternalDisplaySkylightBackupCount() -> usize;
    fn ExternalDisplaySkylightCopyState(display_ids: *mut u32, capacity: usize) -> usize;
    fn ExternalDisplaySkylightRestoreFromState(display_ids: *const u32, count: usize) -> usize;

    fn ExternalDisplayGammaPrepare(display_count: usize) -> u8;
    fn ExternalDisplayGammaFinalize();
    fn ExternalDisplayGammaClearBackups();
    fn ExternalDisplayGammaDisableDisplay(display_id: u32) -> u8;
    fn ExternalDisplayGammaRestoreAll() -> usize;
    fn ExternalDisplayGammaBackupCount() -> usize;
    fn ExternalDisplayGammaCopyStateView(
        index: usize,
        backup_out: *mut CExternalDisplayGammaBackupView,
    ) -> u8;
    fn ExternalDisplayGammaRestoreFromState(
        backups: *const CExternalDisplayGammaBackupView,
        count: usize,
    ) -> usize;
}

pub fn disable() -> ExternalDisplayDisableResult {
    let already_disabled = EXTERNAL_DISPLAYS_DISABLED.load(Ordering::Relaxed);
    let mut result = ExternalDisplayDisableResult {
        ok: true,
        already_disabled,
        total_external: 0,
        disabled: 0,
        failed: 0,
    };

    if already_disabled {
        return result;
    }

    let Some(displays) = online_displays() else {
        result.ok = false;
        return result;
    };

    if !prepare_backends(displays.len()) {
        clear_backups();
        result.ok = false;
        return result;
    }

    for display_id in displays {
        if is_builtin(display_id) {
            continue;
        }

        result.total_external += 1;
        let disabled = unsafe {
            c_bool(ExternalDisplaySkylightDisableDisplay(display_id))
                || c_bool(ExternalDisplayGammaDisableDisplay(display_id))
        };

        if disabled {
            result.disabled += 1;
        } else {
            result.failed += 1;
        }
    }

    finalize_backends();
    EXTERNAL_DISPLAYS_DISABLED.store(result.disabled > 0, Ordering::Relaxed);
    result
}

pub fn restore() -> ExternalDisplayRestoreResult {
    let mut result = ExternalDisplayRestoreResult {
        ok: true,
        had_backups: EXTERNAL_DISPLAYS_DISABLED.load(Ordering::Relaxed) || live_backups_present(),
        restored: 0,
    };

    if !result.had_backups {
        EXTERNAL_DISPLAYS_DISABLED.store(false, Ordering::Relaxed);
        return result;
    }

    unsafe {
        result.restored += ExternalDisplaySkylightRestoreAll();
        result.restored += ExternalDisplayGammaRestoreAll();
    }

    let remaining_backups = live_backups_present();
    EXTERNAL_DISPLAYS_DISABLED.store(remaining_backups, Ordering::Relaxed);
    if remaining_backups {
        result.ok = false;
    }

    result
}

pub fn restore_from_state(state: &ExternalDisplayState) -> ExternalDisplayRestoreResult {
    let mut result = ExternalDisplayRestoreResult {
        ok: true,
        had_backups: state.has_backups(),
        restored: 0,
    };

    if !result.had_backups {
        EXTERNAL_DISPLAYS_DISABLED.store(false, Ordering::Relaxed);
        return result;
    }

    if !state.skylight_display_ids.is_empty() {
        let restored = unsafe {
            ExternalDisplaySkylightRestoreFromState(
                state.skylight_display_ids.as_ptr(),
                state.skylight_display_ids.len(),
            )
        };
        if restored < state.skylight_display_ids.len() {
            result.ok = false;
        }
        result.restored += restored;
    }

    if !state.gamma_backups.is_empty() {
        let views = gamma_backup_views(&state.gamma_backups);
        let restored = unsafe { ExternalDisplayGammaRestoreFromState(views.as_ptr(), views.len()) };
        if restored < state.gamma_backups.len() {
            result.ok = false;
        }
        result.restored += restored;
    }

    EXTERNAL_DISPLAYS_DISABLED.store(false, Ordering::Relaxed);
    result
}

pub fn copy_state() -> Option<ExternalDisplayState> {
    let skylight_display_ids = copy_skylight_state();
    let gamma_backups = copy_gamma_state();
    let state = ExternalDisplayState {
        skylight_display_ids,
        gamma_backups,
    };

    state.has_backups().then_some(state)
}

pub fn are_disabled() -> bool {
    EXTERNAL_DISPLAYS_DISABLED.load(Ordering::Relaxed)
}

fn online_displays() -> Option<Vec<u32>> {
    let mut displays = vec![0_u32; MAX_DISPLAY_COUNT];
    let mut count = 0_usize;
    let ok = unsafe {
        c_bool(ExternalDisplayGetOnlineDisplays(
            displays.as_mut_ptr(),
            displays.len(),
            &mut count,
        ))
    };
    if !ok || count > displays.len() {
        return None;
    }

    displays.truncate(count);
    Some(displays)
}

fn is_builtin(display_id: u32) -> bool {
    unsafe { c_bool(ExternalDisplayIsBuiltin(display_id)) }
}

fn prepare_backends(display_count: usize) -> bool {
    unsafe {
        if !c_bool(ExternalDisplaySkylightPrepare(display_count)) {
            return false;
        }
        if !c_bool(ExternalDisplayGammaPrepare(display_count)) {
            return false;
        }
    }

    true
}

fn finalize_backends() {
    unsafe {
        ExternalDisplaySkylightFinalize();
        ExternalDisplayGammaFinalize();
    }
}

fn clear_backups() {
    unsafe {
        ExternalDisplaySkylightClearBackups();
        ExternalDisplayGammaClearBackups();
    }
}

fn live_backups_present() -> bool {
    unsafe { ExternalDisplaySkylightBackupCount() > 0 || ExternalDisplayGammaBackupCount() > 0 }
}

fn copy_skylight_state() -> Vec<u32> {
    let count = unsafe { ExternalDisplaySkylightBackupCount() };
    if count == 0 {
        return Vec::new();
    }

    let mut display_ids = vec![0_u32; count];
    let copied = unsafe { ExternalDisplaySkylightCopyState(display_ids.as_mut_ptr(), count) };
    display_ids.truncate(copied.min(count));
    display_ids
}

fn copy_gamma_state() -> Vec<GammaBackup> {
    let count = unsafe { ExternalDisplayGammaBackupCount() };
    let mut backups = Vec::with_capacity(count);

    for index in 0..count {
        let mut view = CExternalDisplayGammaBackupView {
            display_id: 0,
            brightness: 0,
            contrast: 0,
            has_brightness: 0,
            has_contrast: 0,
            gamma_sample_count: 0,
            gamma_red: ptr::null(),
            gamma_green: ptr::null(),
            gamma_blue: ptr::null(),
        };
        let ok = unsafe { c_bool(ExternalDisplayGammaCopyStateView(index, &mut view)) };
        if !ok {
            return Vec::new();
        }

        backups.push(gamma_backup_from_view(&view));
    }

    backups
}

fn gamma_backup_from_view(view: &CExternalDisplayGammaBackupView) -> GammaBackup {
    let sample_count = usize::try_from(view.gamma_sample_count).unwrap_or(0);
    let has_gamma_tables = sample_count > 0
        && !view.gamma_red.is_null()
        && !view.gamma_green.is_null()
        && !view.gamma_blue.is_null();

    GammaBackup {
        display_id: view.display_id,
        brightness: c_bool(view.has_brightness).then_some(view.brightness),
        contrast: c_bool(view.has_contrast).then_some(view.contrast),
        gamma_red: if has_gamma_tables {
            unsafe { std::slice::from_raw_parts(view.gamma_red, sample_count).to_vec() }
        } else {
            Vec::new()
        },
        gamma_green: if has_gamma_tables {
            unsafe { std::slice::from_raw_parts(view.gamma_green, sample_count).to_vec() }
        } else {
            Vec::new()
        },
        gamma_blue: if has_gamma_tables {
            unsafe { std::slice::from_raw_parts(view.gamma_blue, sample_count).to_vec() }
        } else {
            Vec::new()
        },
    }
}

fn gamma_backup_views(backups: &[GammaBackup]) -> Vec<CExternalDisplayGammaBackupView> {
    backups
        .iter()
        .map(|backup| {
            let sample_count = valid_gamma_sample_count(backup)
                .and_then(|count| u32::try_from(count).ok())
                .unwrap_or(0);
            let has_gamma = sample_count > 0;

            CExternalDisplayGammaBackupView {
                display_id: backup.display_id,
                brightness: backup.brightness.unwrap_or(0),
                contrast: backup.contrast.unwrap_or(0),
                has_brightness: bool_to_c(backup.brightness.is_some()),
                has_contrast: bool_to_c(backup.contrast.is_some()),
                gamma_sample_count: sample_count,
                gamma_red: if has_gamma {
                    backup.gamma_red.as_ptr()
                } else {
                    ptr::null()
                },
                gamma_green: if has_gamma {
                    backup.gamma_green.as_ptr()
                } else {
                    ptr::null()
                },
                gamma_blue: if has_gamma {
                    backup.gamma_blue.as_ptr()
                } else {
                    ptr::null()
                },
            }
        })
        .collect()
}

fn valid_gamma_sample_count(backup: &GammaBackup) -> Option<usize> {
    let sample_count = backup.gamma_red.len();
    if sample_count == 0
        || backup.gamma_green.len() != sample_count
        || backup.gamma_blue.len() != sample_count
    {
        None
    } else {
        Some(sample_count)
    }
}

fn c_bool(value: u8) -> bool {
    value != 0
}

fn bool_to_c(value: bool) -> u8 {
    if value { 1 } else { 0 }
}
