# LILYGO GNSS adapter

[Backend profile](lilygo-t5-s3-pro.md)

This document describes the current receiver adapter.
The [location contract](../services/location.md) defines its portable observations.

## Configuration

The firmware [entry point](../../firmware/src/main.rs) supplies separate probe, recovery, and observation settings.
The [runtime configuration](../../firmware/src/gnss_worker.rs) contains ordered baud candidates, a per-candidate deadline, and a retry delay.
The [decoder configuration](../../firmware/src/gnss/config.rs) contains only the observation silence limit.
Rates and durations use nonzero types. The probe requires a first candidate.

The [startup probe](../../firmware/src/gnss/probe.rs) opens a fresh serial session for each candidate.
Only a complete sentence with a valid checksum selects a rate. This establishes communication, not receiver identity or a satellite fix.
Each probe is bounded. Firmware retries unsuccessful startup probes after its retry delay.
A successful rate remains in runtime state across suspend and receive retries.
Firmware sends no command to change the receiver's baud.

Silence invalidates observations and ends the receive session. Recovery opens another session at the retained rate.
It does not restart baud probing. Receiver reconfiguration can invalidate that retained choice and currently requires a firmware restart.
The probe is a separate helper. A future board configuration with a known baud can bypass it.

Large satellite storage remains static. The receiver borrows that storage and retains its observation configuration across reset.
NMEA framing is shared by probing and decoding. Each new session discards previous partial sentence state.

## Transport and power

The [firmware runtime](../platform/runtime.md#firmware-execution-placement) supplies the worker's interrupt-executor spawner.
The [runtime worker](../../firmware/src/gnss_worker.rs) composes GNSS with the [serial session contract](../platform/serial.md).
Firmware supplies UART resources, pins, and hardware settings to the [serial owner](../../firmware/src/serial.rs).
GNSS receives only serial sessions and does not configure ESP peripherals.

The [acquisition loop](../../firmware/src/gnss/acquisition.rs) preserves a pending read across observation deadlines.
Read cancellation is permitted only when ending the session, including suspend or silence.
The runtime closes the session before serial sleep preparation. It invalidates receiver observations before acknowledging the pause.
ESP idle-transmitter handling remains in the serial owner.

Firmware owns the [shared auxiliary rail](lilygo-t5-s3-pro-power.md#auxiliary-rail), which also powers LoRa.
Startup admission remains blocked until rail initialization succeeds.
The rail coordinator awaits preparation before cutting power and restores power before releasing acquisition.
The retained baud survives these transitions. Location and UTC observations do not.

The worker remains a concrete Embassy task without heap allocation for sessions or startup probing.
The runtime constructs async UART sessions on the selected executor.

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
Native tests use the published GGA, GSV, and RMC examples and exercise malformed input, UART errors, session expiry, and independent observation expiry.

Hardware checks established receiver communication and satellite signal reports.
Satellite fix acquisition, time to first fix, and measurement accuracy are unverified.

[gnss-protocol]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/Quectel_L76K_GNSS_Protocol_Specification_V1.1.pdf

[gnss]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/GPS/main/main.ino#L80-L96
