use std::{env, fs, process::Command};

#[test]
fn skylight_handles_partial_and_commit_failures() {
    let binary = env::temp_dir().join(format!("lidoff-skylight-test-{}", std::process::id()));
    let compiled = Command::new("cc")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "tests/skylight.c", "-o"])
        .arg(&binary)
        .status()
        .expect("compile Skylight test");
    assert!(compiled.success());

    let result = Command::new(&binary).status();
    fs::remove_file(&binary).unwrap();
    assert!(result.expect("run Skylight test").success());
}
