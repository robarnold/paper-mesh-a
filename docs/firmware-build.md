# Firmware builds

[Development environment](development.md) · [Hardware backends](hardware.md)

Use the [firmware recipes](../justfile) in the devcontainer.
For direct Cargo commands, start in `firmware/` so Cargo loads the [target configuration](../firmware/.cargo/config.toml)
and Rustup selects the [firmware toolchain](../firmware/rust-toolchain.toml).
Passing `--manifest-path` from the repository root selects the manifest but does not load that target configuration.

Use `just image` to prepare a boot image without connecting a device.
The [image script](../scripts/build-image.sh) defines the flash settings and output filenames.
It keeps the ELF with the image so a diagnostic trace can use symbols from the same build.
The development image uses a single factory partition and has no update slot.

## Linker configuration

The upstream chip script gives the empty `.rotext_dummy` padding section a write flag.
The linker combines it with executable code, which makes the resulting segment writable and executable.
The [local override](../firmware/ld/esp32s3-readonly.x) marks the padding read-only while preserving the flash layout.
When upgrading esp-hal, compare the upstream declaration with the override.
Remove the override once the upstream script produces the intended segment permissions.

The [firmware build script](../firmware/build.rs) makes writable executable segments a link error so a regression fails CI.
This check validates ELF permissions. Hardware memory protection requires separate verification.
