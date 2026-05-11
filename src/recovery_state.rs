use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::external_display::ExternalDisplayState;
use crate::logging;

const RECOVERY_STATE_VERSION: u8 = 1;
const RECOVERY_CACHE_DIR: &str = "Library/Caches/co.myrt.lidoff";
const RECOVERY_STATE_FILE: &str = "state.bin";
const LEGACY_RECOVERY_STATE_FILE: &str = "state.plist";

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RecoveryStateData {
    pub pending_brightness_restore: bool,
    pub saved_brightness: f32,
    pub pending_external_restore: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct PersistedState {
    version: u8,
    recovery_state: RecoveryStateData,
    external_state: Option<ExternalDisplayState>,
}

pub fn load() -> Option<(RecoveryStateData, Option<ExternalDisplayState>)> {
    let home = current_home_dir()?;
    if let Err(error) = cleanup_legacy_recovery_file_at(&home) {
        logging::error(format!("failed to remove legacy recovery state: {error}"));
    }

    match load_at_home(&home) {
        Ok(Some(state)) => Some((state.recovery_state, state.external_state)),
        Ok(None) => None,
        Err(error) => {
            logging::error(format!("failed to load recovery state: {error}"));
            None
        }
    }
}

pub fn save(
    recovery_state: &RecoveryStateData,
    external_state: Option<&ExternalDisplayState>,
) -> bool {
    let Some(home) = current_home_dir() else {
        return false;
    };

    save_at_home(&home, recovery_state, external_state).is_ok()
}

pub fn clear() {
    let Some(home) = current_home_dir() else {
        return;
    };

    let path = recovery_state_path(&home);
    let _ = fs::remove_file(path);
}

fn current_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn load_at_home(home: &Path) -> io::Result<Option<PersistedState>> {
    let path = recovery_state_path(home);
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };

    match decode_state(&bytes) {
        Ok(state) => Ok(Some(state)),
        Err(error) => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    }
}

fn save_at_home(
    home: &Path,
    recovery_state: &RecoveryStateData,
    external_state: Option<&ExternalDisplayState>,
) -> io::Result<()> {
    let path = recovery_state_path(home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let bytes = encode_state(recovery_state, external_state)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    fs::write(path, bytes)
}

fn encode_state(
    recovery_state: &RecoveryStateData,
    external_state: Option<&ExternalDisplayState>,
) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&PersistedState {
        version: RECOVERY_STATE_VERSION,
        recovery_state: *recovery_state,
        external_state: external_state.cloned(),
    })
}

fn decode_state(bytes: &[u8]) -> bincode::Result<PersistedState> {
    let state: PersistedState = bincode::deserialize(bytes)?;
    if state.version != RECOVERY_STATE_VERSION {
        return Err(Box::new(bincode::ErrorKind::Custom(format!(
            "unsupported recovery state version {}",
            state.version
        ))));
    }

    Ok(state)
}

fn cleanup_legacy_recovery_file_at(home: &Path) -> io::Result<()> {
    let path = legacy_recovery_state_path(home);
    if !path.exists() {
        return Ok(());
    }

    logging::info(format!(
        "removing obsolete recovery state ({})",
        path.display()
    ));
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
    use crate::external_display::{ExternalDisplayState, GammaBackup};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = env::temp_dir().join(format!("lidoff-test-{}-{timestamp}", process::id()));
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
    fn persisted_state_round_trip_preserves_skylight_and_gamma_state() {
        let recovery_state = RecoveryStateData {
            pending_brightness_restore: true,
            saved_brightness: 0.42,
            pending_external_restore: true,
        };
        let external_state = ExternalDisplayState {
            skylight_display_ids: vec![2, 5],
            gamma_backups: vec![GammaBackup {
                display_id: 7,
                brightness: Some(80),
                contrast: None,
                gamma_red: vec![0.1, 0.3],
                gamma_green: vec![0.2, 0.4],
                gamma_blue: vec![0.5, 0.7],
            }],
        };

        let encoded = encode_state(&recovery_state, Some(&external_state)).unwrap();
        let decoded = decode_state(&encoded).unwrap();

        assert_eq!(decoded.recovery_state, recovery_state);
        assert_eq!(decoded.external_state, Some(external_state));
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
            pending_brightness_restore: false,
            saved_brightness: 0.0,
            pending_external_restore: true,
        };
        let external_state = ExternalDisplayState {
            skylight_display_ids: vec![11],
            gamma_backups: vec![GammaBackup {
                display_id: 13,
                brightness: None,
                contrast: Some(40),
                gamma_red: Vec::new(),
                gamma_green: Vec::new(),
                gamma_blue: Vec::new(),
            }],
        };

        save_at_home(temp.path(), &recovery_state, Some(&external_state)).unwrap();
        let loaded = load_at_home(temp.path()).unwrap().unwrap();

        assert_eq!(loaded.recovery_state, recovery_state);
        assert_eq!(loaded.external_state, Some(external_state));
    }
}
