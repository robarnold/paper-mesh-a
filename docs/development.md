# Development environment

[Project overview](../README.md) · [Documentation guide](../DOCUMENTATION.md)

Use Docker with the Dev Container CLI or an editor that supports devcontainers.
Follow the [README quick start](../README.md#start-development) to run the checks and simulator.

Local development and CI use the same [container image](../.devcontainer/Dockerfile) so both build with the same toolchains.
The tools stay inside the container, which avoids changes to the host shell.
Installed Rust and ESP toolchains stay in the image. Normal builds do not need to write to their installation directories.
The [container configuration](../.devcontainer/devcontainer.json) sets `CARGO_HOME` to `target/cargo-home` in the mounted workspace.
Cargo writes downloads, registry metadata, locks, and user-installed commands there. This data survives local container recreation with the checkout.
Installed image commands remain available through `PATH`. User-installed commands in the writable Cargo home take precedence.
Toolchain changes require an image rebuild. Automatic installation of missing Rust toolchains is disabled.
Local devcontainers still adapt the user IDs to the host without changing ownership of installed tools.

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

Local development and CI reuse devcontainer image layers from GitHub Container Registry to avoid repeated tool installation.
Only successful push runs on `main` publish the image cache. Pull requests can reuse the image layers while checking their own Dockerfile.
Registry access uses the workflow token for repository branches. Forks need anonymous read access to the package to reuse its image cache.
See the [devcontainer action documentation](https://github.com/devcontainers/ci/blob/main/docs/github-action.md) for image cache behavior.

Separate GitHub Actions caches retain native and firmware build outputs in the mounted checkout.
The cache keys include container inputs, Cargo configuration, manifests, and lockfiles because these inputs can change compiled dependencies.
Each revision can save a new cache. A compatible earlier revision supplies build outputs when the exact key is absent.

A separate cache retains Cargo registry indexes, downloaded crate archives, and Git repository data.
It excludes credentials, user-installed commands, and extracted registry sources. Cargo reconstructs extracted sources from the cached archives.
See the [Cargo home documentation](https://doc.rust-lang.org/cargo/guide/cargo-home.html) for cache contents.
Tool installation uses BuildKit cache mounts so its downloaded sources do not become image layers.

The native cache stores the debug profile used by CI. The firmware cache stores both host and Xtensa build outputs.
Generated simulator snapshots and merged firmware images stay outside these caches. CI generates the published artifacts on each run.
Cache misses require a full build. GitHub controls cache access across branches and can evict old entries.
See the [GitHub cache reference](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching) for cache scope and retention.

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
