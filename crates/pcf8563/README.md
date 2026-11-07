# PCF8563 RTC driver

This `no_std` crate accesses the PCF8563 through `embedded-hal` I2C.
It implements the platform calendar contract without ESP peripherals or executor dependencies.
The caller supplies the I2C handle. Firmware selects scheduling and synchronization policy.
The platform owns the synchronization algorithm.

`Pcf8563::new` stores an I2C handle without device access or initialization.
Callers can retain one driver with a dedicated bus or a shared-bus handle.
Firmware borrows its shared bus for one poll and releases the guard before waiting.
Constructing that temporary driver does not reset the RTC or its synchronization state.

The [adapter documentation](../../docs/adapters/rtc.md) defines calendar access, validity, and current verification limits.
