fn main() {
    println!("cargo:rerun-if-changed=src/power_observer.c");
    println!("cargo:rerun-if-changed=src/power_observer.h");
    println!("cargo:rerun-if-changed=src/caffeinate.c");
    println!("cargo:rerun-if-changed=src/caffeinate.h");

    cc::Build::new()
        .file("src/power_observer.c")
        .file("src/caffeinate.c")
        .flag("-fobjc-arc")
        .compile("power");

    println!("cargo:rustc-link-lib=framework=IOKit");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
}
