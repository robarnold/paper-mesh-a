# Development environment

[Project overview](../README.md) · [Documentation guide](../DOCUMENTATION.md)

Use Docker with the Dev Container CLI or an editor that supports devcontainers.
Follow the [README quick start](../README.md#start-development) to run the checks and simulator.

Local development and CI use the same [container image](../.devcontainer/Dockerfile) so both build with the same toolchains.
The tools stay inside the container, which avoids changes to the host shell.
The [container configuration](../.devcontainer/devcontainer.json) restores write access to the tool directories when the container user's numeric ID changes.

## Simulator access

The container binds the simulator to the host loopback interface for local browser access.
Recreate an existing container after changing the port mapping:

```sh
devcontainer up --workspace-folder . --remove-existing-container
```

See the [simulator guide](simulator.md) for controls and the [firmware build guide](firmware-build.md) for target builds.

## Continuous integration

The [CI workflow](../.github/workflows/ci.yml) runs the same [checks](../justfile) that contributors run with `just ci`.
Nextest runs native tests across workspace binaries in parallel. Cargo runs documentation tests separately.
The workflow defines which build artifacts reviewers can download and how long they remain available.
A successful run verifies the build and simulator checks. Firmware startup and peripheral behavior require checks on a physical device.

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
