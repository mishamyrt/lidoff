use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use lidoff_display::{ExternalDisplayState, InternalDisplayState, KeyboardBacklightState};
use serde::{Deserialize, Serialize};

use crate::logging;

const RECOVERY_STATE_VERSION: u8 = 3;
const PRE_KEYBOARD_BACKLIGHT_RECOVERY_STATE_VERSION: u8 = 2;
const LEGACY_RECOVERY_STATE_VERSION: u8 = 1;
const RECOVERY_STATE_FILE: &str = "state.bin";
const LEGACY_RECOVERY_STATE_FILE: &str = "state.plist";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_field_names)]
pub(crate) struct RecoveryStateData {
    pub internal_display_state: Option<InternalDisplayState>,
    pub external_display_state: Option<ExternalDisplayState>,
    pub keyboard_backlight_state: Option<KeyboardBacklightState>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct PersistedState {
    version: u8,
    internal_display_state: Option<InternalDisplayState>,
    external_display_state: Option<ExternalDisplayState>,
    keyboard_backlight_state: Option<KeyboardBacklightState>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct PreKeyboardBacklightPersistedState {
    version: u8,
    internal_display_state: Option<InternalDisplayState>,
    external_display_state: Option<ExternalDisplayState>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
struct LegacyRecoveryStateData {
    pending_brightness_restore: bool,
    saved_brightness: f32,
    pending_external_restore: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct LegacyPersistedState {
    version: u8,
    recovery_state: LegacyRecoveryStateData,
    external_state: Option<ExternalDisplayState>,
}

pub(crate) fn load(recovery_cache_dir: &Path) -> Option<RecoveryStateData> {
    if let Err(error) = cleanup_legacy_recovery_file_at(recovery_cache_dir) {
        logging::error!("failed to remove legacy recovery state: {error}");
    }

    match load_at_cache_dir(recovery_cache_dir) {
        Ok(Some(state)) => Some(state),
        Ok(None) => None,
        Err(error) => {
            logging::error!("failed to load recovery state: {error}");
            None
        }
    }
}

pub(crate) fn save(recovery_cache_dir: &Path, recovery_state: &RecoveryStateData) -> bool {
    save_at_cache_dir(recovery_cache_dir, recovery_state).is_ok()
}

pub(crate) fn clear(recovery_cache_dir: &Path) {
    let path = recovery_state_path(recovery_cache_dir);
    let _ = fs::remove_file(path);
}

fn load_at_cache_dir(recovery_cache_dir: &Path) -> io::Result<Option<RecoveryStateData>> {
    let path = recovery_state_path(recovery_cache_dir);
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };

    match decode_state(&bytes) {
        Ok(state) => Ok(Some(state)),
        Err(error) => Err(io::Error::new(io::ErrorKind::InvalidData, error.to_string())),
    }
}

fn save_at_cache_dir(
    recovery_cache_dir: &Path,
    recovery_state: &RecoveryStateData,
) -> io::Result<()> {
    let path = recovery_state_path(recovery_cache_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let bytes = encode_state(recovery_state)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    fs::write(path, bytes)
}

fn encode_state(recovery_state: &RecoveryStateData) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&PersistedState {
        version: RECOVERY_STATE_VERSION,
        internal_display_state: recovery_state.internal_display_state,
        external_display_state: recovery_state.external_display_state.clone(),
        keyboard_backlight_state: recovery_state.keyboard_backlight_state,
    })
}

fn decode_state(bytes: &[u8]) -> bincode::Result<RecoveryStateData> {
    let Some(&version) = bytes.first() else {
        return Err(Box::new(bincode::ErrorKind::Custom("empty recovery state".to_owned())));
    };

    match version {
        RECOVERY_STATE_VERSION => {
            let state: PersistedState = bincode::deserialize(bytes)?;
            Ok(RecoveryStateData {
                internal_display_state: state.internal_display_state,
                external_display_state: state.external_display_state,
                keyboard_backlight_state: state.keyboard_backlight_state,
            })
        }
        PRE_KEYBOARD_BACKLIGHT_RECOVERY_STATE_VERSION => {
            let state: PreKeyboardBacklightPersistedState = bincode::deserialize(bytes)?;
            Ok(RecoveryStateData {
                internal_display_state: state.internal_display_state,
                external_display_state: state.external_display_state,
                keyboard_backlight_state: None,
            })
        }
        LEGACY_RECOVERY_STATE_VERSION => {
            let state: LegacyPersistedState = bincode::deserialize(bytes)?;
            Ok(RecoveryStateData {
                internal_display_state: if state.recovery_state.pending_brightness_restore
                    && state.recovery_state.saved_brightness >= 0.0
                {
                    Some(InternalDisplayState {
                        brightness: state.recovery_state.saved_brightness,
                    })
                } else {
                    None
                },
                external_display_state: if state.recovery_state.pending_external_restore {
                    state.external_state
                } else {
                    None
                },
                keyboard_backlight_state: None,
            })
        }
        _ => Err(Box::new(bincode::ErrorKind::Custom(format!(
            "unsupported recovery state version {version}",
        )))),
    }
}

fn cleanup_legacy_recovery_file_at(recovery_cache_dir: &Path) -> io::Result<()> {
    let path = legacy_recovery_state_path(recovery_cache_dir);
    if !path.exists() {
        return Ok(());
    }

    logging::info!("removing obsolete recovery state ({})", path.display());
    fs::remove_file(path)
}

fn recovery_state_path(recovery_cache_dir: &Path) -> PathBuf {
    recovery_cache_dir.join(RECOVERY_STATE_FILE)
}

fn legacy_recovery_state_path(recovery_cache_dir: &Path) -> PathBuf {
    recovery_cache_dir.join(LEGACY_RECOVERY_STATE_FILE)
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        RecoveryStateData, cleanup_legacy_recovery_file_at, decode_state, encode_state,
        legacy_recovery_state_path, load_at_cache_dir, save_at_cache_dir,
    };
    use lidoff_display::{ExternalDisplayState, InternalDisplayState, KeyboardBacklightState};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let path =
                env::temp_dir().join(format!("lidoff-test-{}-{timestamp}", process::id()));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn persisted_state_round_trip_preserves_internal_and_external_display_state() {
        let recovery_state = RecoveryStateData {
            internal_display_state: Some(InternalDisplayState { brightness: 0.42 }),
            external_display_state: Some(ExternalDisplayState {
                skylight_display_ids: vec![2, 5].into(),
            }),
            keyboard_backlight_state: Some(KeyboardBacklightState { brightness: 0.65 }),
        };

        let encoded = encode_state(&recovery_state).unwrap();
        let decoded = decode_state(&encoded).unwrap();

        assert_eq!(decoded, recovery_state);
    }

    #[test]
    fn legacy_recovery_file_is_removed() {
        let temp = TestDir::new();
        let legacy_path = legacy_recovery_state_path(temp.path());
        fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        fs::write(&legacy_path, b"legacy").unwrap();

        cleanup_legacy_recovery_file_at(temp.path()).unwrap();

        assert!(!legacy_path.exists());
    }

    #[test]
    fn save_and_load_round_trip_uses_binary_state_file() {
        let temp = TestDir::new();
        let recovery_state = RecoveryStateData {
            internal_display_state: Some(InternalDisplayState { brightness: 0.73 }),
            external_display_state: Some(ExternalDisplayState {
                skylight_display_ids: vec![11].into(),
            }),
            keyboard_backlight_state: Some(KeyboardBacklightState { brightness: 0.84 }),
        };

        save_at_cache_dir(temp.path(), &recovery_state).unwrap();
        let loaded = load_at_cache_dir(temp.path()).unwrap().unwrap();

        assert_eq!(loaded, recovery_state);
    }

    #[test]
    fn legacy_v1_state_decodes_into_optional_display_state() {
        let encoded = bincode::serialize(&super::LegacyPersistedState {
            version: super::LEGACY_RECOVERY_STATE_VERSION,
            recovery_state: super::LegacyRecoveryStateData {
                pending_brightness_restore: true,
                saved_brightness: 0.58,
                pending_external_restore: true,
            },
            external_state: Some(ExternalDisplayState {
                skylight_display_ids: vec![17].into(),
            }),
        })
        .unwrap();

        let decoded = decode_state(&encoded).unwrap();

        assert_eq!(
            decoded,
            RecoveryStateData {
                internal_display_state: Some(InternalDisplayState { brightness: 0.58 }),
                external_display_state: Some(ExternalDisplayState {
                    skylight_display_ids: vec![17].into(),
                }),
                keyboard_backlight_state: None,
            }
        );
    }

    #[test]
    fn legacy_v2_state_decodes_without_keyboard_backlight_state() {
        let encoded = bincode::serialize(&super::PreKeyboardBacklightPersistedState {
            version: super::PRE_KEYBOARD_BACKLIGHT_RECOVERY_STATE_VERSION,
            internal_display_state: Some(InternalDisplayState { brightness: 0.58 }),
            external_display_state: Some(ExternalDisplayState {
                skylight_display_ids: vec![17].into(),
            }),
        })
        .unwrap();

        let decoded = decode_state(&encoded).unwrap();

        assert_eq!(
            decoded,
            RecoveryStateData {
                internal_display_state: Some(InternalDisplayState { brightness: 0.58 }),
                external_display_state: Some(ExternalDisplayState {
                    skylight_display_ids: vec![17].into(),
                }),
                keyboard_backlight_state: None,
            }
        );
    }
}
