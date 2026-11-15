# Power observations

This `no_std` crate samples platform `BatteryGauge` and `ChargerObserver` capabilities independently.
It supplies direct sampling and a cooperative worker loop with pause acknowledgement and failure-transition logging.

The runtime supplies device capabilities, monotonic milliseconds, a polling delay, and static service storage.
The crate owns Embassy task storage and returns prepared `Workers` for runtime-selected execution.
The service owns cached state, notifications, and pause storage. Preparation returns `(Lifecycle, Workers)`. Callers already have the service reference.
The service reference implements platform `Power`. The service also provides change notifications.
Firmware retains device construction and executor placement.
The crate has no ESP, I2C, chip-driver, or timer-driver dependency.
Task preparation allocates one boxed future to hide device types. Polling adds no worker allocations.
See [power observation ownership](../../docs/adapters/power-observation.md).

## Dependency notices

The [manifest](Cargo.toml) pins existing project dependencies.
Embassy and log permit MIT or Apache-2.0 licensing. This project uses their MIT terms.
Include the [Embassy notice](../platform/licenses/LICENSE-embassy-MIT) and [log notice](../esp-adapters/licenses/log-MIT.txt) with distributed binaries.

The service uses the existing `static_cell` dependency for one-time pause initialization.
Include its [MIT notice](../esp-adapters/licenses/static_cell-MIT.txt) with distributed binaries.

The service uses `CriticalSectionRawMutex` internally. Native tests enable the standard-library critical-section implementation.
