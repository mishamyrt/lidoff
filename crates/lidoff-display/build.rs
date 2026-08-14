fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let native_sources = [
        "src/shim/brightness.c",
        "src/shim/cursor.c",
        "src/shim/displays.c",
        "src/shim/keyboard_backlight.c",
        "src/shim/skylight.c",
    ];
    let native_headers = [
        "src/shim/brightness.h",
        "src/shim/cursor.h",
        "src/shim/displays.h",
        "src/shim/keyboard_backlight.h",
        "src/shim/skylight.h",
    ];

    for source in &native_sources {
        println!("cargo:rerun-if-changed={source}");
    }
    for header in &native_headers {
        println!("cargo:rerun-if-changed={header}");
    }

    cc::Build::new()
        .files(native_sources)
        .include("macos")
        .std("c11")
        .opt_level_str("s")
        .flag("-Wall")
        .flag("-Wextra")
        .define("NDEBUG", None)
        .compile("lidoff_native");

    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=CoreGraphics");
    println!("cargo:rustc-link-lib=framework=IOKit");
    println!("cargo:rustc-link-lib=objc");
}
