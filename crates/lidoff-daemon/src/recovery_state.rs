use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use lidoff_display::{ExternalDisplayState, InternalDisplayState, KeyboardBacklightState};
use rkyv::{Archive, Deserialize, Serialize, rancor::Error, util::AlignedVec};

use crate::logging;

const RECOVERY_STATE_FILE: &str = "state.bin";
const LEGACY_RECOVERY_STATE_FILE: &str = "state.plist";

#[derive(Clone, Debug, Default, PartialEq, Archive, Serialize, Deserialize)]
#[allow(clippy::struct_field_names)]
#[rkyv(attr(allow(clippy::struct_field_names)))]
pub(crate) struct RecoveryStateData {
    pub internal_display_state: Option<InternalDisplayState>,
    pub external_display_state: Option<ExternalDisplayState>,
    pub keyboard_backlight_state: Option<KeyboardBacklightState>,
}

#[derive(Archive, Serialize, Deserialize)]
struct PersistedState {
    version: String,
    boot_session_uuid: Option<String>,
    recovery_state: RecoveryStateData,
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

pub(crate) fn save(
    recovery_cache_dir: &Path,
    recovery_state: &RecoveryStateData,
) -> io::Result<()> {
    save_at_cache_dir(recovery_cache_dir, recovery_state)
}

pub(crate) fn clear(recovery_cache_dir: &Path) -> io::Result<()> {
    let path = recovery_state_path(recovery_cache_dir);
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn load_at_cache_dir(recovery_cache_dir: &Path) -> io::Result<Option<RecoveryStateData>> {
    let path = recovery_state_path(recovery_cache_dir);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };

    match decode_state(&bytes, boot_session_uuid()) {
        Ok(state) => Ok(Some(state)),
        Err(error) => {
            logging::info!("discarding obsolete or invalid recovery state: {error}");
            fs::remove_file(path)?;
            Ok(None)
        }
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

    let bytes = encode_state(recovery_state, boot_session_uuid())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    fs::write(path, bytes)
}

fn boot_session_uuid() -> Option<&'static str> {
    static BOOT_SESSION_UUID: OnceLock<Option<String>> = OnceLock::new();
    BOOT_SESSION_UUID
        .get_or_init(|| {
            let session = Command::new("/usr/sbin/sysctl")
                .args(["-n", "kern.bootsessionuuid"])
                .output()
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());
            if session.is_none() {
                logging::error!(
                    "failed to determine boot session; retaining unverified recovery state"
                );
            }
            session
        })
        .as_deref()
}

fn encode_state(
    recovery_state: &RecoveryStateData,
    boot_session: Option<&str>,
) -> Result<AlignedVec, Error> {
    rkyv::to_bytes::<Error>(&PersistedState {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        boot_session_uuid: boot_session.map(str::to_owned),
        recovery_state: recovery_state.clone(),
    })
}

fn decode_state(bytes: &[u8], boot_session: Option<&str>) -> io::Result<RecoveryStateData> {
    let state = rkyv::from_bytes::<PersistedState, Error>(bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    if state.version != env!("CARGO_PKG_VERSION") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported recovery state version {}", state.version),
        ));
    }

    let mut recovery_state = state.recovery_state;
    // Display IDs belong to a boot session.
    if boot_session.is_some() && state.boot_session_uuid.as_deref() != boot_session {
        recovery_state.external_display_state = None;
    }
    Ok(recovery_state)
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

        let encoded = encode_state(&recovery_state, Some("boot-a")).unwrap();
        let decoded = decode_state(&encoded, Some("boot-a")).unwrap();

        assert_eq!(decoded, recovery_state);

        let mut after_reboot = recovery_state.clone();
        after_reboot.external_display_state = None;
        assert_eq!(decode_state(&encoded, Some("boot-b")).unwrap(), after_reboot);
        assert_eq!(decode_state(&encoded, None).unwrap(), recovery_state);

        let unverified = encode_state(&recovery_state, None).unwrap();
        assert_eq!(decode_state(&unverified, Some("boot-a")).unwrap(), after_reboot);
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
    fn persisted_state_round_trip_preserves_empty_and_spilled_display_state() {
        for recovery_state in [
            RecoveryStateData::default(),
            RecoveryStateData {
                external_display_state: Some(ExternalDisplayState {
                    skylight_display_ids: vec![1, 2, 3, 4, 5].into(),
                }),
                ..RecoveryStateData::default()
            },
        ] {
            let encoded = encode_state(&recovery_state, Some("boot-a")).unwrap();
            assert_eq!(decode_state(&encoded, Some("boot-a")).unwrap(), recovery_state);
        }
    }

    #[test]
    fn cache_from_another_app_version_is_removed() {
        let temp = TestDir::new();
        let path = super::recovery_state_path(temp.path());
        let encoded = rkyv::to_bytes::<rkyv::rancor::Error>(&super::PersistedState {
            version: "old-version".to_owned(),
            boot_session_uuid: None,
            recovery_state: RecoveryStateData::default(),
        })
        .unwrap();
        fs::write(&path, encoded).unwrap();

        assert!(load_at_cache_dir(temp.path()).unwrap().is_none());
        assert!(!path.exists());
        assert!(load_at_cache_dir(temp.path()).unwrap().is_none());
    }

    #[test]
    fn invalid_cache_is_removed() {
        let temp = TestDir::new();
        let path = super::recovery_state_path(temp.path());
        let encoded = encode_state(&RecoveryStateData::default(), Some("boot-a")).unwrap();
        let old_cache = [3, 0, 1, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 6, 0, 0, 0, 0];

        for bytes in [&[][..], &old_cache, b"invalid cache", &encoded[..encoded.len() - 1]] {
            assert!(decode_state(bytes, Some("boot-a")).is_err());
            fs::write(&path, bytes).unwrap();
            assert!(load_at_cache_dir(temp.path()).unwrap().is_none());
            assert!(!path.exists());
        }
    }
}
