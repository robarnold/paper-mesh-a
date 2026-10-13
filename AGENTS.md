# Repository agent instructions

All project tooling must run inside the devcontainer. Do not install Rust,
ESP tools, Python packages, or other project dependencies on the host. Do not
modify the host home directory or shell startup files.

The host may run Docker and the devcontainer CLI to manage the container, and
read or edit repository files. Use these commands from the repository root:

```sh
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . just ci
```

If the container cannot start, report the failure and ask how to proceed.
Do not fall back to running checks on the host.

Run `just ci` inside the container before committing. Do not use semantic commit
messages. Do not flash a connected device unless the user requests it.

The intended device is the LILYGO T5 S3 E-Paper Pro with LoRa. The firmware in
`firmware/` uses Rust, esp-hal, and Embassy on esp-rtos. Run Cargo from that
directory so it loads the target configuration and the `esp` toolchain.
Custom display/touch handling, power management, and multicore control are planned.
ESPHome and an ESP-IDF application runtime are not part of the chosen design.

The root Cargo workspace builds the shared `no_std` application in `crates/app/`
and the native Rust simulator in `simulator/`. Firmware is intentionally excluded
from that workspace to isolate Xtensa settings and ESP dependency features.
Keep application behavior and rendering in the shared crate; simulator HTTP and
wall-clock pacing belong in the native binary. Hardware drivers implement the
application's platform interfaces using esp-hal/embedded-hal underneath.
Keep `docs/architecture.md` current when changing shared application contracts.
Rendering and touch use the shared portrait rotation; native display storage
stays 960 × 540 while the application sees 540 × 960.
Run `just sim` in the devcontainer to serve the simulator on localhost:8080.

Keep vendor documents external: link to pinned upstream sources instead of
committing copies, extracted text, or document screenshots. The reference index
is `docs/vendor/lilygo/t5-s3-pro/README.md`; download documents into the
devcontainer's `/tmp` directory when needed. Record project findings in our own
docs with links and page/section references to the original sources.
