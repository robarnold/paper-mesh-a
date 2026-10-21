# LILYGO GNSS adapter

[Backend profile](lilygo-t5-s3-pro.md)

This document describes the current receiver adapter.
The [location contract](../services/location.md) defines its portable observations.

## Transport and power

[`firmware/src/gnss`](../../firmware/src/gnss/mod.rs) enables PCA9535 `IO0_0` through the shared I2C mutex.
It sets the port 0 output latch before it selects the output direction.
It preserves other port 0 bits and all display controls on port 1.
The auxiliary supply stays enabled after startup, during receiver faults, and after exit from the GNSS screen.
Thus, this version also supplies power to LoRa. The LoRa driver stays uninitialized.
The manual suspend experiment drops the UART and disables this supply after the active display update ends.
It retains an idle transmitter during the pause to keep UART1 clocks available for HAL sleep preparation.
This transmitter has no attached pins or receive wake lock.
Resume enables the supply and restarts baud selection. Previous GNSS observations cannot supply a retained fix.

The adapter uses UART1 on RX44/TX43 with 8 data bits, no parity, and one stop bit.
UART1 uses the GPIO matrix. Application logs continue through USB Serial/JTAG.

The worker uses a priority-1 interrupt executor on core 0 to receive bytes during application rendering.
The receive threshold is 32 bytes. The software sentence buffer holds 128 bytes.
The adapter sends no receiver commands.

Baud selection tries 9600 and 38400 baud for four seconds each.
A complete NMEA sentence with a valid envelope and checksum selects the baud rate.
Baud selection does not establish the receiver model.
After ten seconds without a valid NMEA sentence, the adapter tries both rates again.
The [application](../application.md#location-screen) selects observation age limits.
Incorrect checksums and malformed sentences do not change the previous observation time.

## Position and elevation

The decoder accepts standard GNSS talker IDs and converts GGA coordinates to signed millionths of a degree.
It converts the GGA altitude field to elevation above mean sea level in signed millimeters.
The field must specify meters with `M`. Missing, malformed, and overflowing values supply no elevation.
The decoder discards fractional precision below one millimeter. It does not add geoid separation to the reported elevation.

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

The decoder converts knots to millimeters per second and course to thousandths of a degree.
It discards fractional precision below one thousandth before conversion. Speed conversion rounds to the nearest millimeter per second.

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

It supports 12 report streams, 36 satellites per stream, and 96 distinct satellites in the summary.
These buffers use static RAM instead of the interrupt stack.
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
