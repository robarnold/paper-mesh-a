# paper-mesh-a

This project provides shared Rust application software for e-paper devices.
It includes a native simulator and an initial firmware backend for the
LILYGO T5 S3 E-Paper Pro with LoRa.
The firmware uses `no_std` Rust, esp-hal, and Embassy on esp-rtos.
It uses both cores at 240 MHz and starts the asynchronous timer.
It sends a boot message and a heartbeat every 30 seconds through USB Serial/JTAG.
It supplies passive Wi-Fi and Bluetooth LE scans through the shared discovery interface.
It supplies GNSS coordinates, elevation, fix status, satellite signals, and receiver activity through the shared location interface.
The GNSS screen also shows speed, course relative to true north, and UTC time of day.
The GNSS supply stays enabled. Receiver communication passed hardware checks. A satellite fix is unverified.
An optional diagnostic build requests both scans and prints their results through USB Serial/JTAG.
It renders the shared portrait interface on the ED047TC1 panel.
Two framebuffers use 518,400 bytes of PSRAM.

GT911 input supplies taps to the shared counter.
GPIO interrupts service touch on core 0 while core 1 executes display refreshes.
Count changes and heartbeat updates use partial display refreshes.
Each tap changes only the counter pixels. Startup requires a full refresh.
Rapid taps combine into the next frame while the current refresh completes.
System power management, packet radio protocols, and OTA stay planned work.
The [LILYGO backend profile](docs/boards/lilygo-t5-s3-pro.md) gives information about the board
and hardware checks. USB boot, heartbeat, display, and basic touch checks passed on 2026-10-17.
Other hardware checks appear in that profile.

The native Rust simulator runs the shared application with a grayscale display,
touch input, and a controllable clock. It runs in the devcontainer.
It serves its screen at [localhost:8080](http://localhost:8080).

The [application architecture](docs/architecture.md) gives information about shared contracts
and implementation boundaries. The [device service design](docs/device-interfaces.md)
defines portable contracts and simulator scenarios.
[Backend profiles](docs/hardware.md) map hardware to these contracts.
The [display contract](docs/display-constraints.md) defines component requirements.
[LILYGO display evidence](docs/boards/lilygo-t5-s3-pro-display.md) records panel
limits, driver limits, and calculations that support the design.

## Development

The host must have Docker and the Dev Container CLI, or an editor with
Dev Containers support. Run all project tools in the container:

```sh
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . just ci
```

The image contains a native Rust compiler and the Espressif Xtensa Rust toolchain,
registered as `esp`. It also contains `espup`, `esp-generate`, `espflash`, and
`just`. `.devcontainer/Dockerfile` specifies their versions.
`firmware/rust-toolchain.toml` selects the installed `esp` toolchain.
`firmware/Cargo.lock` fixes the firmware dependency versions.

The native Rust compiler builds the simulator and shared application tests
in the container. The root `Cargo.lock` fixes their dependency versions.

The image build installs the tools. Rustup, Cargo, and Espressif files stay
in the container filesystem. The container does not mount the host home
directory. `espup` writes its environment script to `/opt/esp/export.sh`.
The image sets the necessary paths for interactive and non-interactive commands.
No host shell changes or manual script execution are necessary.

Dev Containers can change the container user's numeric ID to match the host or
GitHub runner. The creation hook then updates ownership of the tool directories.
Cargo and Rustup keep write access after the ID change.

## Simulator

```sh
devcontainer exec --workspace-folder . just sim
```

Open [localhost:8080](http://localhost:8080).
Click the screen or use the equivalent device controls.
Select `Radio debugger` to see simulated Wi-Fi and Bluetooth LE results.
Select `GNSS` to see simulated coordinates, satellite signals, and receiver activity.
The [GNSS scenarios](docs/simulator.md#gnss-screen) include fix loss, expired data, unavailable data, and receiver faults.
The debugger sorts results by signal strength.
The simulator guide describes the [scan scenarios](docs/simulator.md#radio-debugger).
Use Pause and the time controls to inspect refreshes and heartbeats.
To stop the server, press Ctrl+C.

The container publishes port 8080 only on the host's loopback interface.
If an existing container predates this port setting, recreate it with
`devcontainer up --workspace-folder . --remove-existing-container`.

The browser transports input and displays PNG frames. Rust code in the
container controls application behavior, hit testing, rendering, and simulated
hardware. The simulator does not execute Xtensa or ULP instructions.
It does not emulate ESP registers.
Firmware uses the same application, renderer, and portrait transform.
Hardware drivers supply display output, touch input, and passive radio scans.

The [display driver notice](crates/ed047tc1/README.md) records its source and GPL license.

```sh
# Run native Clippy and deterministic application and device scenarios.
devcontainer exec --workspace-folder . just native-check

# Render a screenshot without a web server or display.
devcontainer exec --workspace-folder . just simulator-snapshot
```

The snapshot command writes `target/simulator/hello.png` for the portrait view
and `target/simulator/native.png` for the native framebuffer.
During simulation, [/frame-native.png](http://localhost:8080/frame-native.png)
provides the native image directly.
The [application architecture](docs/architecture.md) gives information about the shared contracts.
The [simulator guide](docs/simulator.md) gives information about the API.

## Firmware commands

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

The [radio backend profile](docs/boards/lilygo-t5-s3-pro.md#radio-discovery)
specifies scan behavior, limits, and the optional `radio-scan-demo` feature.
The default build waits for application requests.
The screen controls request scans through the same shared application.

### Linker configuration

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

## Continuous integration

GitHub Actions uses a GitHub-hosted `ubuntu-24.04` runner to build the same
devcontainer and execute `just ci`.
The runner does not install or execute the Rust or ESP toolchain outside the
container. CI checks infrastructure syntax, Rust formatting, and Clippy for
both targets. It also runs native simulation scenarios, links release firmware,
makes the boot image, and renders simulator screenshots.

GitHub uploads the firmware image, ELF, and checksums as artifacts.
It also uploads a separate simulator screenshot artifact.
GitHub keeps these artifacts for 14 days.
These checks do not verify hardware execution.

CI operates without a project image registry. It does not publish container images.
The first image build downloads and compiles development tools.
Local builds reuse Docker layers.
If build time becomes a problem, add a shared CI image cache.

## Hardware access

The default container operates without a connected device and also operates in CI.
When hardware arrives, add an ignored `.devcontainer/local/devcontainer.json`
with USB and serial passthrough for the host.
Select that file with the CLI's `--config` option.
Do not install flashing tools on the host.
Hardware setup and serial permissions must pass checks with the actual device.

For Linux, map only the selected serial device into the container.
Add its numeric group ID with Docker's `--group-add` option.
The ignored local configuration can use `PAPER_MESH_SERIAL_PORT` and
`PAPER_MESH_SERIAL_GID` for these values.
Keep the actual `/dev/ttyACM*` name inside the container.
Espflash uses this name to identify the USB interface.
Use the default container for the simulator on host port 8080.
After device identification, run these commands on the host:

```sh
export PAPER_MESH_SERIAL_PORT=$(readlink -f /dev/serial/by-id/REPLACE_WITH_CONFIRMED_DEVICE)
export PAPER_MESH_SERIAL_GID=$(stat -Lc '%g' "$PAPER_MESH_SERIAL_PORT")
devcontainer up --workspace-folder . --config .devcontainer/local/devcontainer.json
devcontainer exec --workspace-folder . --config .devcontainer/local/devcontainer.json \
  espflash list-ports --list-all-ports --skip-update-check
```

If USB reconnection changes the device node, recreate the local container.
Keep the same `--config` option and add `--remove-existing-container` to `devcontainer up`.

Before the first flash, follow the [factory backup procedure](docs/boards/lilygo-t5-s3-pro.md#factory-backup).
