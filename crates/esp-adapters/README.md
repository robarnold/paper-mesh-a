# ESP adapters

This `no_std` crate implements platform contracts through ESP hardware and Embassy.
The `esp32s3` feature exposes the [USB adapter](../../docs/adapters/usb.md).
Firmware supplies device configuration, command dispatch, diagnostics, an executor, and an allocator.
The adapter does not depend on firmware or the application.

Default features are empty. Native workspace checks run the adapter's session and lifecycle unit tests without ESP dependencies.
Firmware explicitly enables `esp32s3` and builds the adapter with its Xtensa target configuration.
The two workspaces retain separate lockfiles. The native lockfile records optional hardware dependencies without compiling them.

## Dependency notices

The [manifest](Cargo.toml) pins the existing direct dependency versions.
All direct third-party dependencies offer MIT or Apache-2.0 licensing.
The selected MIT terms permit distribution with their copyright and permission notices.
Include these notices with device images that contain this adapter:

| Dependency and source | MIT notice |
| --- | --- |
| [Embassy](https://github.com/embassy-rs/embassy) executor, futures, synchronization, time, and USB | [Embassy](licenses/embassy-MIT.txt) |
| [esp-hal](https://github.com/esp-rs/esp-hal) | [esp-hal](licenses/esp-hal-MIT.txt) |
| [heapless](https://github.com/rust-embedded/heapless) | [heapless](licenses/heapless-MIT.txt) |
| [log](https://github.com/rust-lang/log) | [log](licenses/log-MIT.txt) |
| [static_cell](https://github.com/embassy-rs/static-cell) | [static_cell](licenses/static_cell-MIT.txt) |
