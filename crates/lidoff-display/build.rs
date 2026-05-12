use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("missing out dir"));
    let sdk_root = xcrun_sdk_root();

    let native_sources = [
        manifest_dir.join("src/shim/brightness.c"),
        manifest_dir.join("src/shim/displays.c"),
        manifest_dir.join("src/shim/keyboard_backlight.c"),
        manifest_dir.join("src/shim/skylight.c"),
    ];
    let native_headers = [
        manifest_dir.join("src/shim/brightness.h"),
        manifest_dir.join("src/shim/displays.h"),
        manifest_dir.join("src/shim/keyboard_backlight.h"),
        manifest_dir.join("src/shim/skylight.h"),
    ];

    for source in &native_sources {
        println!("cargo:rerun-if-changed={}", source.display());
    }
    for header in &native_headers {
        println!("cargo:rerun-if-changed={}", header.display());
    }

    let header_dir = manifest_dir.join("macos");

    let mut objects = Vec::with_capacity(native_sources.len());
    for source in &native_sources {
        let object = out_dir.join(
            source
                .file_name()
                .expect("missing source name")
                .to_string_lossy()
                .replace(".c", ".o"),
        );
        compile_c(source, &object, &header_dir, &sdk_root);
        objects.push(object);
    }

    let archive = out_dir.join("liblidoff_native.a");
    create_archive(&archive, &objects);

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=lidoff_native");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=CoreGraphics");
    println!("cargo:rustc-link-lib=framework=IOKit");
    println!("cargo:rustc-link-lib=objc");
}

fn xcrun_sdk_root() -> String {
    let output =
        Command::new("xcrun").arg("--show-sdk-path").output().expect("failed to run xcrun");
    assert!(output.status.success(), "xcrun --show-sdk-path failed");
    String::from_utf8(output.stdout).expect("sdk path is not utf-8").trim().to_owned()
}

fn compile_c(source: &Path, object: &Path, header_dir: &Path, sdk_root: &str) {
    let status = Command::new("clang")
        .arg("-c")
        .arg("-std=c11")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Os")
        .arg("-DNDEBUG")
        .arg("-isysroot")
        .arg(sdk_root)
        .arg("-I")
        .arg(header_dir)
        .arg("-o")
        .arg(object)
        .arg(source)
        .status()
        .unwrap_or_else(|err| panic!("failed to compile {}: {err}", source.display()));

    assert!(status.success(), "clang failed for {}", source.display());
}

fn create_archive(archive: &Path, objects: &[PathBuf]) {
    let mut command = Command::new("/usr/bin/libtool");
    command.arg("-static").arg("-o").arg(archive);
    for object in objects {
        command.arg(object);
    }

    let status =
        command.status().unwrap_or_else(|err| panic!("failed to archive objc objects: {err}"));
    assert!(status.success(), "libtool failed");
}
