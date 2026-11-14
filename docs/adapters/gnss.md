# Portable GNSS receiver

The [GNSS crate](../../crates/gnss/README.md) converts NMEA sentences into platform location observations.
It owns bounded framing, satellite aggregation, decoding, and observation expiry.
The [location contract](../services/location.md) defines its output.
The [board integration](../boards/lilygo-t5-s3-pro-gnss.md) owns hardware choices and runtime policy.

## Acquisition

The caller supplies observation timing through [Config](../../crates/gnss/src/config.rs).
It owns [ReceiverStorage](../../crates/gnss/src/receiver.rs), which the receiver borrows to avoid large construction temporaries.
Reset retains configuration and storage ownership while discarding partial input and previous observations.

The [acquisition future](../../crates/gnss/src/acquisition.rs) accepts an embedded I/O reader, monotonic timestamps, deadline futures, and a publication callback.
It preserves a pending read across observation expiry. Silence, EOF, and read errors end acquisition.
The caller then closes the serial session before retrying.
Acquisition does not select baud rates, reset hardware, or perform system suspend.

The separate [probe](../../crates/gnss/src/probe.rs) accepts a platform serial factory, nonempty baud candidates, and a deadline factory.
It opens a fresh session for each candidate and accepts a complete checksum-valid envelope without requiring a position fix.
The result identifies a working baud, not a receiver model. The caller decides whether to probe and when to retry.
GNSS sends no command to change the receiver configuration.

The crate uses Embassy future combinators but supplies no executor, task storage, or timer driver.
It performs no heap allocation. Firmware provides static satellite storage and runtime timing callbacks.

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

See the [GNSS decoder](../../crates/gnss/src/receiver.rs) for field conversion and rounding.

RMC UTC supplies hours, minutes, seconds, and the date from the same sentence.
The decoder discards fractional seconds and interprets the two-digit year as 2000 through 2099.
It validates Gregorian dates, including leap days.
An invalid or absent date cannot supply a clock reference, but it does not discard valid time of day or motion.
The [RTC adapter](rtc.md) uses complete references for synchronization.
Invalid or absent numeric fields clear only their readout. Invalid RMC status also clears UTC, even if its time field has numbers.
This conservative policy does not display time from invalid RMC reports before a fix.

RMC motion and UTC expire independently of GGA and GSV data. Other messages cannot extend their receipt times.
The receiver does not request VTG or ZDA output.

## Satellite reports and receiver activity

The GSV decoder separates reports by constellation and signal ID.
It accepts the optional signal ID and both `BD` and `GB` identifiers for BeiDou.
It requires complete, ordered groups before publication. Duplicate satellite IDs invalidate a group.
It preserves the previous complete group until replacement or expiry after five seconds.
Partial groups also expire after five seconds.

The decoder bounds the number of streams and satellites to keep their storage off the interrupt stack.
See the [GSV implementation](../../crates/gnss/src/gsv.rs) for capacities.

Storage overflow marks summary counts as lower bounds.
Multiple signals for one satellite contribute one count and the greatest reported C/N0.
An empty C/N0 field stays unknown. A reported zero stays zero.

Receiver activity counts messages with a supported talker, a valid envelope, and a correct checksum.
Malformed measurement fields can establish activity, but they cannot refresh a fix or a satellite report.
UART errors clear position, motion, UTC, and satellite data. The last response time and message count stay available.

## Sources and verification limits

[Quectel L76K protocol specification V1.1][gnss-protocol], section 2.2.2, pages 12–13, specifies GGA fields and fix quality.
Page 13 defines altitude above mean sea level and the separate geoid separation field.
Appendix C, page 41, specifies default GGA output at 1 Hz and 9600 baud.

Section 2.2.3, pages 14–15, specifies GSV groups, signal IDs, and C/N0 in dB-Hz.
It excludes the combined `GN` identifier from GSV reports.
Section 2.2.1, pages 9–11, specifies RMC fields, data status, mode, and the published example.
Section 2.2.5, page 17, defines course over ground relative to true north.
Native [decoder tests](../../crates/gnss/tests/receiver.rs) use the published GGA, GSV, and RMC examples and exercise malformed input, UART errors, session expiry, and independent observation expiry.

[Session tests](../../crates/gnss/tests/sessions.rs) cover probing, cancellation, failed opens, and reads retained across expiry deadlines.
The native tests do not establish satellite reception, fix accuracy, or physical serial and wake behavior.

[gnss-protocol]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/Quectel_L76K_GNSS_Protocol_Specification_V1.1.pdf
