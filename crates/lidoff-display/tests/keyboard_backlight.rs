use std::{env, fs, process::Command};

#[test]
fn keyboard_backlight_drains_temporary_objects_on_success_and_failure() {
    let binary = env::temp_dir().join(format!("lidoff-backlight-test-{}", std::process::id()));
    let compiled = Command::new("cc")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "tests/keyboard_backlight.m",
            "-framework",
            "Foundation",
            "-lobjc",
            "-o",
        ])
        .arg(&binary)
        .status()
        .expect("compile keyboard backlight test");
    assert!(compiled.success());

    let result = Command::new(&binary).status();
    fs::remove_file(&binary).unwrap();
    assert!(result.expect("run keyboard backlight test").success());
}
