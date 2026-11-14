use std::{fs, os::unix::fs::PermissionsExt, process::Command, time::SystemTime};

#[test]
fn status_distinguishes_loaded_and_running_services() {
    let id = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
    let tools = std::env::temp_dir().join(format!("lidoff-status-{id}"));
    fs::create_dir_all(&tools).unwrap();
    let launchctl = tools.join("launchctl");
    fs::write(
        &launchctl,
        "#!/bin/sh\n[ \"$1\" = print ] || exit 1\nprintf 'state = %s\\n' \"$LIDOFF_TEST_SERVICE_STATE\"\n",
    )
    .unwrap();
    fs::set_permissions(&launchctl, fs::Permissions::from_mode(0o755)).unwrap();

    for (state, expected) in
        [("exited", "service installed but not running"), ("running", "service is running")]
    {
        let output = Command::new(env!("CARGO_BIN_EXE_lidoff"))
            .arg("status")
            .env("PATH", &tools)
            .env("LIDOFF_TEST_SERVICE_STATE", state)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    }
    fs::remove_dir_all(tools).unwrap();
}
