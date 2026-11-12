# ESP adapters

This `no_std` crate implements platform contracts through ESP hardware and Embassy.
The `esp32s3` feature exposes the [USB adapter](../../docs/adapters/usb.md), [RTC clock adapter](../../docs/adapters/rtc.md#esp-clock-integration), and [touch adapter](../../docs/adapters/touch.md).
Firmware supplies device configuration, command dispatch, diagnostics, an executor, and an allocator.
USB preparation returns a typed worker set. Firmware selects a local spawner for the set before using the lifecycle handle.
The adapters do not depend on firmware or the application.
The reusable [GPIO readiness provider](../../docs/adapters/gpio-readiness.md) separates acquisition notifications from reset, wake control, and worker execution.
Touch converts GT911 reports into the platform [contact-frame contract](../../docs/services/input.md#contact-frame-contract).
The RTC adapter receives synchronization policy, state storage, the shared ESP I2C bus, and location observations from the runtime.
The runtime selects RTC and touch executors and bus queue capacity.
RTC and touch own their tasks and box their worker futures once during preparation to hide the bus capacity.
RTC workers implement the platform `WorkerSet` contract. Firmware selects the spawner.

Default features are empty. Native workspace checks run the adapter's session, lifecycle, and readiness unit tests without ESP dependencies.
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
| [embedded-graphics](https://github.com/embedded-graphics/embedded-graphics) | [embedded-graphics](licenses/embedded-graphics-MIT.txt) |
| [heapless](https://github.com/rust-embedded/heapless) | [heapless](licenses/heapless-MIT.txt) |
| [log](https://github.com/rust-lang/log) | [log](licenses/log-MIT.txt) |
| [static_cell](https://github.com/embassy-rs/static-cell) | [static_cell](licenses/static_cell-MIT.txt) |
