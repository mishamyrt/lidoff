use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use lidoff_display::{ExternalDisplayState, InternalDisplayState};
use serde::{Deserialize, Serialize};

use crate::logging;

const RECOVERY_STATE_VERSION: u8 = 2;
const LEGACY_RECOVERY_STATE_VERSION: u8 = 1;
const RECOVERY_CACHE_DIR: &str = "Library/Caches/co.myrt.lidoff";
const RECOVERY_STATE_FILE: &str = "state.bin";
const LEGACY_RECOVERY_STATE_FILE: &str = "state.plist";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct RecoveryStateData {
    pub internal_display_state: Option<InternalDisplayState>,
    pub external_display_state: Option<ExternalDisplayState>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct PersistedState {
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

pub(crate) fn load() -> Option<RecoveryStateData> {
    let home = current_home_dir()?;
    if let Err(error) = cleanup_legacy_recovery_file_at(&home) {
        logging::error(format!("failed to remove legacy recovery state: {error}"));
    }

    match load_at_home(&home) {
        Ok(Some(state)) => Some(state),
        Ok(None) => None,
        Err(error) => {
            logging::error(format!("failed to load recovery state: {error}"));
            None
        }
    }
}

pub(crate) fn save(recovery_state: &RecoveryStateData) -> bool {
    let Some(home) = current_home_dir() else {
        return false;
    };

    save_at_home(&home, recovery_state).is_ok()
}

pub(crate) fn clear() {
    let Some(home) = current_home_dir() else {
        return;
    };

    let path = recovery_state_path(&home);
    let _ = fs::remove_file(path);
}

fn current_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn load_at_home(home: &Path) -> io::Result<Option<RecoveryStateData>> {
    let path = recovery_state_path(home);
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

fn save_at_home(home: &Path, recovery_state: &RecoveryStateData) -> io::Result<()> {
    let path = recovery_state_path(home);
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
            })
        }
        _ => Err(Box::new(bincode::ErrorKind::Custom(format!(
            "unsupported recovery state version {version}",
        )))),
    }
}

fn cleanup_legacy_recovery_file_at(home: &Path) -> io::Result<()> {
    let path = legacy_recovery_state_path(home);
    if !path.exists() {
        return Ok(());
    }

    logging::info(format!("removing obsolete recovery state ({})", path.display()));
    fs::remove_file(path)
}

fn recovery_state_dir(home: &Path) -> PathBuf {
    home.join(RECOVERY_CACHE_DIR)
}

fn recovery_state_path(home: &Path) -> PathBuf {
    recovery_state_dir(home).join(RECOVERY_STATE_FILE)
}

fn legacy_recovery_state_path(home: &Path) -> PathBuf {
    recovery_state_dir(home).join(LEGACY_RECOVERY_STATE_FILE)
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
        legacy_recovery_state_path, load_at_home, save_at_home,
    };
    use lidoff_display::{ExternalDisplayState, InternalDisplayState};

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
                skylight_display_ids: vec![2, 5],
            }),
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
                skylight_display_ids: vec![11],
            }),
        };

        save_at_home(temp.path(), &recovery_state).unwrap();
        let loaded = load_at_home(temp.path()).unwrap().unwrap();

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
            external_state: Some(ExternalDisplayState { skylight_display_ids: vec![17] }),
        })
        .unwrap();

        let decoded = decode_state(&encoded).unwrap();

        assert_eq!(
            decoded,
            RecoveryStateData {
                internal_display_state: Some(InternalDisplayState { brightness: 0.58 }),
                external_display_state: Some(ExternalDisplayState {
                    skylight_display_ids: vec![17],
                }),
            }
        );
    }
}
