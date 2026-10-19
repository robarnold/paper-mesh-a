# Repository agent instructions

Follow the [documentation guide](DOCUMENTATION.md) for all project prose.
This includes documentation, comments, review text, commit descriptions, and agent responses.

## Development environment

Run project tools in the devcontainer, except version control tools.
The host can run version control tools. Do not install Rust, ESP tools,
Python packages, or other project dependencies on the host. Do not change the
host home directory or shell startup files.

The host can run Docker and the devcontainer CLI to control the container.
The host can also read and edit repository files. Use these commands from the
repository root:

```sh
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . just ci
```

If the container cannot start, report the failure. Ask the user how to continue.
Do not run checks on the host as an alternative.

Before a commit, run `just ci` in the container. Do not use semantic commit
messages. Do not flash a connected device unless the user requests it.

## Implementation boundaries

The intended device is the LILYGO T5 S3 E-Paper Pro with LoRa. The firmware in
`firmware/` uses Rust, esp-hal, and Embassy on esp-rtos. Run Cargo from that
directory to load the target configuration and the `esp` toolchain.

The firmware implements display output and single-contact touch input.
Display refresh runs on core 1. Touch interrupts and the application run on core 0.
System power management stays planned work.
The design does not use ESPHome or an ESP-IDF application runtime.

The root Cargo workspace builds the `no_std` application in `crates/app/`
and the application toolkit in `crates/platform/`.
It also builds the native Rust simulator in `simulator/`.
It also tests the ED047TC1 encoder and GT911 protocol in separate crates.
Firmware stays outside that
workspace to isolate Xtensa settings and ESP dependency features.

Keep application screens, workflows, layouts, and policy choices in `crates/app/`.
Put common functionality and reusable application components in `crates/platform/`.
This toolkit includes controls, gestures, rendering helpers, timing, formatting, service contracts, and coordinate adapters.
Organize toolkit modules by responsibility.
Keep application actions, fixed layouts, and policy values out of reusable components.
Do not add an application dependency to the platform crate or backend adapters.

Keep native dimensions and resource control in the backend.
Keep simulator HTTP and wall-clock pacing in the native binary.
Hardware adapters implement platform contracts through esp-hal and embedded-hal.
Runtime code connects the application to these adapters.

Adapters convert board details into capabilities, geometry, and logical controls.
Do not add board or chip checks to shared application policy.

The current demonstration uses the shared portrait rotation. Native display
storage stays 960 × 540, while the application sees 540 × 960.
Run `just sim` in the devcontainer to serve the simulator on localhost:8080.
