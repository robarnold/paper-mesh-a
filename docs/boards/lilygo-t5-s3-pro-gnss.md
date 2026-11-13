# LILYGO GNSS adapter

[Backend profile](lilygo-t5-s3-pro.md)

This document describes the current receiver adapter.
The [location contract](../services/location.md) defines its portable observations.

## Configuration

The firmware [entry point](../../firmware/src/main.rs) supplies `GNSS_CONFIG` to the receiver worker.
The [configuration type](../../firmware/src/gnss/config.rs) contains ordered baud candidates, probe and silence durations, and the hardware retry delay.
`BaudRates` requires a first candidate and permits additional candidates in static storage.
Rates use `NonZeroU32`. Durations use `NonZeroU64` milliseconds. These types exclude empty candidate lists and zero values without runtime validation.

Acquisition starts with the first candidate and advances through the list when the probe deadline expires.
A valid sentence replaces the probe deadline with the silence deadline. Noise does not extend either deadline.
Resume restarts acquisition with the same configuration. UART-construction failures use the configured retry delay.

Large satellite storage remains static. `Receiver::new` requires configuration and borrows that storage without constructing a large stack temporary.
The receiver always has configuration. An in-place reset clears prior observations while retaining configuration and the storage borrow.
UART FIFO settings, sentence capacities, and satellite-group expiry remain implementation details of their existing modules.

## Transport and power

The [firmware runtime](../platform/runtime.md#firmware-execution-placement) supplies the GNSS worker's interrupt-executor spawner.
It selects core 0 and priority 1 so UART consumption can proceed during synchronous application rendering.

Firmware owns the [shared auxiliary rail](lilygo-t5-s3-pro-power.md#auxiliary-rail), which also powers LoRa.
GNSS `prepare` returns workers and their lifecycle handle without launching a task or requesting a pause.
Firmware requests the initial pause before `Workers::spawn`, then enables the rail before releasing acquisition admission.
The worker has no supply interface, board power-register access, or I2C bus reference.

The auxiliary supply stays enabled after startup, during receiver faults, and after exit from the GNSS screen.
Thus, this version also supplies power to LoRa. The LoRa driver stays uninitialized.
The manual suspend experiment prepares GNSS before firmware disables this supply, after the active display update ends.
It retains an idle transmitter during the pause to keep UART1 clocks available for HAL sleep preparation.
This transmitter has no attached pins or receive wake lock.
Firmware restores the supply before releasing GNSS to restart baud selection. Previous GNSS observations cannot supply a retained fix.

GNSS retains a concrete Embassy task and performs no heap allocation for startup or supply operations.
The async UART is created inside its selected executor. Large satellite storage remains static.

The [receiver worker](../../firmware/src/gnss/mod.rs) listens during rendering and tries the rates used by the supported receiver variants.
Only a complete sentence with a valid checksum selects a rate. This identifies a working transport but cannot identify the fitted receiver.
After a prolonged silence the adapter retries baud selection without sending a receiver command.

The [application](../application.md#location-screen) selects observation age limits.
Incorrect checksums and malformed sentences do not change the previous observation time.

## Position and elevation

Use the reported altitude above mean sea level only when the sentence specifies meters.
Adding geoid separation again would change the reference of the displayed elevation. See section 2.2.2 of the [Quectel protocol][gnss-protocol].

Elevation requires a valid position fix. An unavailable elevation does not discard valid horizontal coordinates.
It accepts satellite fix qualities 1, 2, 4, and 5.
Other defined qualities supply no position fix, including estimated, manual, and simulated positions.
A UART error clears the partial sentence and invalidates an established observation.
The next valid GGA sentence restores the service.

## Motion and UTC

RMC status `A` or `D` permits speed, course, and UTC.
An optional mode field must specify satellite mode `A`, `D`, `F`, or `R`.
Invalid status and other modes clear these readouts. The decoder accepts legacy RMC sentences without the mode field.
The optional navigation status does not replace the data status. The vendor example uses navigation status `V` with valid data.

See the [GNSS decoder](../../firmware/src/gnss/receiver.rs) for field conversion and rounding.

RMC UTC supplies hours, minutes, seconds, and the date from the same sentence.
The decoder discards fractional seconds and interprets the two-digit year as 2000 through 2099.
It validates Gregorian dates, including leap days.
An invalid or absent date cannot supply a clock reference, but it does not discard valid time of day or motion.
The [RTC adapter](lilygo-t5-s3-pro-rtc.md) uses complete references for synchronization.
Invalid or absent numeric fields clear only their readout. Invalid RMC status also clears UTC, even if its time field has numbers.
This conservative policy does not display time from invalid RMC reports before a fix.

RMC motion and UTC expire independently of GGA and GSV data. Other messages cannot extend their receipt times.
The adapter does not request VTG or ZDA output. Receiver power stays enabled outside manual suspend.

## Satellite reports and receiver activity

The GSV decoder separates reports by constellation and signal ID.
It accepts the optional signal ID and both `BD` and `GB` identifiers for BeiDou.
It requires complete, ordered groups before publication. Duplicate satellite IDs invalidate a group.
It preserves the previous complete group until replacement or expiry after five seconds.
Partial groups also expire after five seconds.

The decoder bounds the number of streams and satellites to keep their storage off the interrupt stack.
See the [GSV implementation](../../firmware/src/gnss/gsv.rs) for capacities.

Storage overflow marks summary counts as lower bounds.
Multiple signals for one satellite contribute one count and the greatest reported C/N0.
An empty C/N0 field stays unknown. A reported zero stays zero.

Receiver activity counts messages with a supported talker, a valid envelope, and a correct checksum.
Malformed measurement fields can establish activity, but they cannot refresh a fix or a satellite report.
UART errors clear position, motion, UTC, and satellite data. The last response time and message count stay available.
The USB heartbeat includes these diagnostics without coordinates.

## Sources and verification limits

[Quectel L76K protocol specification V1.1][gnss-protocol], section 2.2.2, pages 12–13, specifies GGA fields and fix quality.
Page 13 defines altitude above mean sea level and the separate geoid separation field.
Appendix C, page 41, specifies default GGA output at 1 Hz and 9600 baud.
The [vendor example][gnss] supplies the second baud rate and auxiliary-supply sequence.

Section 2.2.3, pages 14–15, specifies GSV groups, signal IDs, and C/N0 in dB-Hz.
It excludes the combined `GN` identifier from GSV reports.
Section 2.2.1, pages 9–11, specifies RMC fields, data status, mode, and the published example.
Section 2.2.5, page 17, defines course over ground relative to true north.
Native tests use the published GGA, GSV, and RMC examples and exercise malformed input, UART errors, baud retries, and independent expiry.

Hardware checks established receiver communication and satellite signal reports.
Satellite fix acquisition, time to first fix, and measurement accuracy are unverified.

[gnss-protocol]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/Quectel_L76K_GNSS_Protocol_Specification_V1.1.pdf

[gnss]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/GPS/main/main.ino#L80-L96
