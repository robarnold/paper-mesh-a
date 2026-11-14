# BQ25896 charger

This `no_std` crate accesses the BQ25896 through `embedded-hal` I2C.
The `observation` module returns platform `ChargerState` values without changing charger configuration.
The separate `ship` module provides guarded battery isolation and startup restoration.

Callers hold the bus for each complete operation. Firmware selects polling, shutdown order, and recovery after uncertain hardware access.
`observation::Observer` implements platform `ChargerObserver` through platform `Arbitrated` bus ownership and holds its guard for the complete observation.
The crate has no ESP or executor dependency.
See the [adapter documentation](../../docs/adapters/bq25896.md) for operation contracts and verification limits.

## Dependency notice

The [manifest](Cargo.toml) pins the existing `embedded-hal` dependency.
[embedded-hal](https://github.com/rust-embedded/embedded-hal/tree/embedded-hal-v1.0.0) permits MIT or Apache-2.0 licensing.
This project uses its MIT terms. Include the [MIT notice](licenses/embedded-hal-MIT.txt) with binaries that contain this crate.

The manifest also pins the existing `embassy-sync` dependency for its raw mutex bound.
Embassy permits MIT or Apache-2.0 licensing. Include the existing [MIT notice](../platform/licenses/LICENSE-embassy-MIT).
