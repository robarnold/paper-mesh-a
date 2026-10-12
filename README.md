# paper-mesh-a

Development infrastructure for Rust firmware on the LILYGO T5 S3 E-Paper Pro
with LoRa. Device firmware, display and touch support, power management, radio
protocol integration, and OTA will be bootstrapped separately.

## Development

The only host prerequisites are Docker and the Dev Container CLI, or an editor
with Dev Containers support. Run all project tooling inside the container:

```sh
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . just ci
```

The image contains a pinned host Rust compiler, the Espressif Xtensa Rust
toolchain registered as `esp`, `espup`, `esp-generate`, `espflash`, and `just`.
Version pins live in `.devcontainer/Dockerfile`. Use `cargo +esp` for ESP32-S3
firmware and the default host compiler for host tools and tests.

Tool installation happens when building the image. Rustup, Cargo, and Espressif
files stay inside the container filesystem; the host home directory is not
mounted. `espup` writes its environment script to `/opt/esp/export.sh`, and the
image sets the required paths for interactive and non-interactive commands.
No host shell startup changes or manual sourcing are needed.

Dev Containers may adjust the container user's numeric ID to match the host or
GitHub runner. The creation hook updates ownership of the container's tool
directories so Cargo and Rustup remain writable after that adjustment.

## Continuous integration

GitHub Actions uses the GitHub-hosted `ubuntu-24.04` runner to build this same
devcontainer and execute `just ci`. The runner itself does not install or invoke
the Rust/ESP toolchain. CI checks infrastructure syntax and formatting, then
cross-compiles `tools/toolchain-smoke` as a `no_std` library for the ESP32-S3.
This verifies compiler installation and code generation, not firmware linking,
flashing, board support, or hardware operation.

CI does not require a project image registry or publish images. The first image
build downloads and compiles the development tools; local rebuilds reuse Docker
layers. Add shared CI image caching if build time becomes an issue.

## Hardware access

The default container requires no connected device, so it also works in CI.
When hardware arrives, add an ignored `.devcontainer/devcontainer.local.json`
with USB/serial passthrough for the specific host and select it with the CLI's
`--config` option. Do not install flashing tools on the host. Hardware setup will
be verified against the actual device and serial permissions at that point.
