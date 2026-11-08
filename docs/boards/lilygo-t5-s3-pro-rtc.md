# T5 S3 Pro real-time clock

[Board profile](lilygo-t5-s3-pro.md) · [Time contract](../services/time.md)

## Hardware integration

This board uses the [PCF8563 calendar adapter](../adapters/rtc.md).
That document defines device access and calendar validity.
The platform [time service](../services/time.md#calendar-synchronization) defines synchronization and cache behavior.
The [firmware task](../../firmware/src/rtc.rs) runs the [ESP clock adapter](../adapters/rtc.md#esp-clock-integration) with the shared I2C bus.
Runtime lifecycle composition connects the clock handle to suspend coordination.
The [runtime](../../firmware/src/main.rs) selects synchronization policy and owns the cache and pause storage.
Clock and worker endpoints share that configured state. The worker does not select age limits or correction thresholds.
The [board profile](lilygo-t5-s3-pro.md#shared-buses-power-domains-and-wake-routing) describes shared resources.
Physical retention requires a suitable backup source on `VRTC`.

## GNSS source

Embassy supplies monotonic timestamps and polling deadlines through `embassy-time`.
It does not supply retained calendar time. The [Embassy time contract](https://docs.embassy.dev/embassy-time/0.5.1/default/index.html#wall-clock-time) distinguishes these responsibilities.
The platform synchronization service owns calendar validity, correction consumption, and cache age limits.

The [GNSS adapter](lilygo-t5-s3-pro-gnss.md#motion-and-utc) supplies complete RMC date and time observations for synchronization.
It interprets the RMC year as 2000 through 2099.
It does not infer a century from the build date.
It ignores fractional RMC seconds and does not use PPS.

## Evidence and limits

Physical I2C access, backup retention, oscillator accuracy, and GPS synchronization remain unverified on this board.
Native adapter tests do not establish those hardware properties.
