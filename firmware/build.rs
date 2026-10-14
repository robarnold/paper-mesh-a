use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    for script in ["paper-mesh.x", "esp32s3-readonly.x"] {
        let source = PathBuf::from("ld").join(script);
        println!("cargo:rerun-if-changed={}", source.display());
        fs::copy(source, out_dir.join(script)).expect("copy the ESP32-S3 linker script");
    }

    // Select the corrected chip script explicitly.
    // esp-hal and xtensa-lx-rt supply memory regions and shared section scripts.
    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rustc-link-arg=-Tpaper-mesh.x");
    // A future writable executable segment must fail the build, including CI.
    println!("cargo:rustc-link-arg=-Wl,--warn-rwx-segments,--error-rwx-segments");
}
