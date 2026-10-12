# paper-mesh-a

Rust firmware for the LILYGO T5 S3 E-Paper Pro with LoRa. The bootstrap uses
`no_std` Rust, esp-hal, and Embassy on the Rust esp-rtos scheduler. It initializes
core 0 at 80 MHz, starts the async timer, and logs a boot message followed by a
heartbeat every 30 seconds over USB Serial/JTAG.

Display, touch, PSRAM allocation, power management, second-core tasks, radio
protocol integration, and OTA are still to come. The board profile and bring-up
notes are in [docs/hardware.md](docs/hardware.md). Hardware operation has not yet
been tested.

## Development

The only host prerequisites are Docker and the Dev Container CLI, or an editor
with Dev Containers support. Run all project tooling inside the container:

```sh
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . just ci
```

The image contains a pinned host Rust compiler, the Espressif Xtensa Rust
toolchain registered as `esp`, `espup`, `esp-generate`, `espflash`, and `just`.
Version pins live in `.devcontainer/Dockerfile`. `firmware/rust-toolchain.toml`
selects the installed `esp` toolchain; `firmware/Cargo.lock` locks dependencies.
The default host-target compiler remains available inside the container for
future host tools and tests.

Tool installation happens when building the image. Rustup, Cargo, and Espressif
files stay inside the container filesystem; the host home directory is not
mounted. `espup` writes its environment script to `/opt/esp/export.sh`, and the
image sets the required paths for interactive and non-interactive commands.
No host shell startup changes or manual sourcing are needed.

Dev Containers may adjust the container user's numeric ID to match the host or
GitHub runner. The creation hook updates ownership of the container's tool
directories so Cargo and Rustup remain writable after that adjustment.

## Firmware commands

From the repository root on the host:

```sh
# Format Rust sources.
devcontainer exec --workspace-folder . just fmt

# Run Clippy for the ESP32-S3 target.
devcontainer exec --workspace-folder . just firmware-check

# Link release firmware and package its boot image without accessing a device.
devcontainer exec --workspace-folder . just image
```

Recipes change into `firmware/` before running Cargo so it reads that directory's
`.cargo/config.toml`. When using Cargo directly, run it there inside the
container; passing only `--manifest-path` from the repository root does not load
the firmware's target configuration.

`just image` writes these files to `target/artifacts/`:

- `paper-mesh-firmware.elf`: firmware with debug symbols for diagnostics.
- `paper-mesh-firmware.bin`: merged bootloader, partition table, and application,
  starting at flash offset `0x0`, configured for 16 MB QIO flash at 80 MHz.
- `SHA256SUMS`: checksums for both artifacts.

The image uses espflash's bundled bootloader and default single-factory partition
table. It is a development image, with no OTA slots or update transport. The
`esp-bootloader-esp-idf` crate supplies image metadata and bootloader support;
the application does not run ESP-IDF or FreeRTOS. No `cargo run` flash runner is
configured, and none of the recipes flash a device.

### Linker configuration

`firmware/ld/esp32s3-readonly.x` is a versioned copy of the esp-hal 1.2.2 chip
script with one correction: `.rotext_dummy` is declared `READONLY` instead of
`NOLOAD`. This empty padding section prevents overlap between the data and
instruction mappings of flash. The upstream declaration gives it a write flag,
which combines with the adjacent executable `.text` section to produce an RWX
load segment.

The corrected section still has no file contents (`NOBITS`) and retains its
address, size, and alignment. The flash code segment is now read/execute. The
small `paper-mesh.x` entry script selects this correction while retaining the
HAL's memory regions and shared section scripts. Provenance and the upstream
license are included in `firmware/ld/`; compare the chip script with upstream
when upgrading esp-hal, and remove the override when it is fixed upstream.

`build.rs` enables RWX diagnostics and makes them linker errors, so builds and
CI reject a recurrence. These ELF flags describe the linker's output; they do
not configure hardware memory protection on the ESP32-S3.

## Continuous integration

GitHub Actions uses the GitHub-hosted `ubuntu-24.04` runner to build this same
devcontainer and execute `just ci`. The runner itself does not install or invoke
the Rust/ESP toolchain. `just ci` checks infrastructure syntax, Rust formatting,
and Clippy, then links release firmware and packages its boot image. This replaces
the earlier toolchain-only smoke test with a complete firmware build. GitHub
uploads the image, ELF, and checksums as a workflow artifact retained for 14 days.
These checks verify the build and packaging, not execution on hardware.

CI does not require a project image registry or publish images. The first image
build downloads and compiles the development tools; local rebuilds reuse Docker
layers. Add shared CI image caching if build time becomes an issue.

## Hardware access

The default container requires no connected device, so it also works in CI.
When hardware arrives, add an ignored `.devcontainer/devcontainer.local.json`
with USB/serial passthrough for the specific host and select it with the CLI's
`--config` option. Do not install flashing tools on the host. Hardware setup will
be verified against the actual device and serial permissions at that point.
