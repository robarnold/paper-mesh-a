# Development environment

[Project overview](../README.md) · [Documentation guide](../DOCUMENTATION.md)

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

The native Rust compiler builds the simulator and tests both shared crates in the container. The root `Cargo.lock` fixes their dependency versions.

The image build installs the tools. Rustup, Cargo, and Espressif files stay
in the container filesystem. The container does not mount the host home
directory. `espup` writes its environment script to `/opt/esp/export.sh`.
The image sets the necessary paths for interactive and non-interactive commands.
No host shell changes or manual script execution are necessary.

Dev Containers can change the container user's numeric ID to match the host or
GitHub runner. The creation hook then updates ownership of the tool directories.
Cargo and Rustup keep write access after the ID change.

## Simulator access

The container publishes port 8080 only on the host's loopback interface.
If an existing container predates this port setting, recreate it with this command:

```sh
devcontainer up --workspace-folder . --remove-existing-container
```

The [simulator guide](simulator.md) gives the startup command and controls.
The [firmware build guide](firmware-build.md) gives build commands, output paths, and linker details.

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
For device access, add an ignored `.devcontainer/local/devcontainer.json` with USB and serial passthrough.
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
