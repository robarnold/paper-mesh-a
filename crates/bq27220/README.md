# BQ27220 fuel gauge

This `no_std` crate reads BQ27220 observations through `embedded-hal` I2C and returns the platform `BatteryState`.
The caller supplies the bus and holds it for the complete sample.
Firmware owns polling, publication, shared-bus arbitration, and power coordination.
The crate does not configure, calibrate, unseal, or reset the gauge.

The [observation documentation](../../docs/adapters/bq27220.md) describes validity checks and verification limits.

## Dependency notice

The [manifest](Cargo.toml) pins the existing `embedded-hal` dependency.
[embedded-hal](https://github.com/rust-embedded/embedded-hal/tree/embedded-hal-v1.0.0) permits MIT or Apache-2.0 licensing.
This project uses its MIT terms. Include the [MIT notice](licenses/embedded-hal-MIT.txt) with binaries that contain this crate.
