# Firmware builds

[Development environment](development.md) · [Hardware backends](hardware.md)

Run these commands from the repository root on the host:

```sh
# Format Rust sources.
devcontainer exec --workspace-folder . just fmt

# Run Clippy for the ESP32-S3 target.
devcontainer exec --workspace-folder . just firmware-check

# Link release firmware and make its boot image without device access.
devcontainer exec --workspace-folder . just image
```

Recipes change into `firmware/` before Cargo execution.
Thus, Cargo reads that directory's `.cargo/config.toml`.
For direct Cargo commands, use that directory in the container.
The `--manifest-path` option alone does not load the firmware target
configuration from the repository root.

`just image` writes these files to `target/artifacts/`:

- `paper-mesh-firmware.elf`: firmware with debug symbols for diagnostics.
- `paper-mesh-firmware.bin`: merged bootloader, partition table, and application.
  The image starts at flash offset `0x0`. It selects DIO mode for 16 MB flash at 80 MHz.
- `SHA256SUMS`: checksums for both artifacts.

The image uses espflash's supplied bootloader and default single-factory
partition table. This development image has no OTA slots or update transport.
The `esp-bootloader-esp-idf` crate supplies image metadata and bootloader support.
The application does not run ESP-IDF or FreeRTOS.
There is no `cargo run` flash runner. No recipe flashes a device.

The [radio backend profile](boards/lilygo-t5-s3-pro-radio.md)
specifies scan behavior, limits, and the optional `radio-scan-demo` feature.
The default build waits for application requests.
Application controls request scans through the portable discovery interface.

The [USB backend profile](boards/lilygo-t5-s3-pro-usb.md#diagnostics-and-recovery) specifies CDC logs, ROM recovery, and flashing after OTG startup.
The [download command](boards/lilygo-t5-s3-pro-usb.md#enter-rom-download-mode) permits software entry when the application and display are idle.

## Linker configuration

`firmware/ld/esp32s3-readonly.x` is a versioned copy of the esp-hal 1.2.2 chip
script with one correction. It declares `.rotext_dummy` as `READONLY` instead of
`NOLOAD`. This empty padding section prevents overlap between the data and
instruction mappings of flash.
The upstream declaration gives this section a write flag.
With the adjacent executable `.text` section, this flag produces an RWX load
segment.

The corrected section has no file contents (`NOBITS`).
Its address, size, and alignment stay the same.
The flash code segment now has read and execute permissions.
The `paper-mesh.x` entry script selects this correction.
It keeps the HAL memory regions and shared section scripts.

`firmware/ld/` includes the source details and upstream license.
When esp-hal changes, compare the chip script with upstream.
When upstream corrects the declaration, remove the override.

`build.rs` enables RWX diagnostics and makes them linker errors.
Thus, local builds and CI reject a new RWX load segment.
These ELF flags give information about linker output.
They do not configure ESP32-S3 hardware memory protection.
