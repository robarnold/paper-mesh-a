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
