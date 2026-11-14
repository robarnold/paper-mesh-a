# BQ27220 fuel gauge

This `no_std` crate reads BQ27220 observations through `embedded-hal` I2C and returns the platform `BatteryState`.
`Bq27220` implements platform `BatteryGauge` through platform `Arbitrated` bus ownership and holds its guard for the complete sample.
The low-level `battery` function accepts an already acquired I2C bus.
Firmware owns polling, publication, shared-bus arbitration, and power coordination.
The crate does not configure, calibrate, unseal, or reset the gauge.

The [observation documentation](../../docs/adapters/bq27220.md) describes validity checks and verification limits.

## Dependency notice

The [manifest](Cargo.toml) pins the existing `embedded-hal` dependency.
[embedded-hal](https://github.com/rust-embedded/embedded-hal/tree/embedded-hal-v1.0.0) permits MIT or Apache-2.0 licensing.
This project uses its MIT terms. Include the [MIT notice](licenses/embedded-hal-MIT.txt) with binaries that contain this crate.

The manifest also pins the existing `embassy-sync` dependency for its raw mutex bound.
Embassy permits MIT or Apache-2.0 licensing. Include the existing [MIT notice](../platform/licenses/LICENSE-embassy-MIT).
