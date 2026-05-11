fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/lid_sensor.h");
    println!("cargo:rerun-if-changed=src/lid_sensor.c");

    cc::Build::new()
        .file("src/lid_sensor.c")
        .include("src")
        .flag("-fobjc-arc")
        .compile("lidoff_lidsensor");

    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=IOKit");
}
