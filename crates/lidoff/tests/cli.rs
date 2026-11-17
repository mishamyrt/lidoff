use std::process::Command;

#[test]
fn no_arguments_enter_the_monitor_startup_path() {
    let output =
        Command::new(env!("CARGO_BIN_EXE_lidoff")).env_remove("HOME").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&output.stderr), "lidoff: home directory not found\n");
}
