# Location and time

[Device services](../device-interfaces.md)

## Current location interface

The implemented `Location` interface supplies provider availability and observations.
These observations include WGS84 coordinates, elevation, motion, UTC time of day, receiver activity, and satellite signals.
Coordinates and elevation are optional.

Elevation uses signed millimeters above mean sea level.
Speed uses millimeters per second. Course uses thousandths of a degree clockwise from true north.
Course must be less than 360 degrees. Zero speed and midnight UTC are valid values.
Position, motion, UTC, satellite data, and receiver activity have independent receipt times and validity.

UTC supplies hours, minutes, and seconds without a date or an accuracy estimate.
The observation does not set a system clock.

`Location::snapshot(now_ms)` returns a copied snapshot without a wait or an allocation.
Repeated reads keep the original receipt times.
Availability distinguishes startup, a ready provider, unavailable data, and a fault.
The contract does not identify hardware or simulator providers.

`Coordinates::new` checks WGS84 latitude and longitude limits in signed millionths of a degree.
Zero coordinates are valid. An absent value means no position fix.
`observed_at_ms` records local receipt time in the monotonic epoch.
It does not specify UTC or the source measurement time.

`ReceiverActivity` supplies the last valid message time and a cumulative message count.
The count saturates at `u32::MAX` and does not reset after a transport fault.
`SatelliteObservation` supplies counts and up to `MAX_SATELLITES` entries in descending signal order.
The current capacity is eight entries.
Each entry has a system, identifier, and optional C/N0 value in dB-Hz.

An absent signal level differs from a reported zero.
Neither adds to the count of satellites with a signal.
The adapter counts each satellite once across signal types and uses the oldest receipt time among complete reports.
After storage overflow, `truncated` marks the counts as lower bounds.
Satellites in view can include satellites without measured signal levels.

The [location screen](../application.md#location-screen) selects age limits, display labels, and display deadlines.

## Planned location and time services

The remaining fields and operations below stay proposed work.

`Location` separates provider availability, acquisition state, and fix validity.
Observations can include latitude, longitude, elevation, speed, course, accuracy,
and measurement time. Elevation must have a reference.
Provider metadata is optional.
Each field must have units and validity.
`(0, 0)` is valid data, not a missing-fix marker.

Local receipt time and source measurement time are separate.
A cached observation must not appear as a new fix.
Position validity and UTC validity are independent.
Course during movement does not replace device orientation.

Timing precision depends on the source and synchronization path.
A received message does not necessarily identify an accurate clock edge.
Adapters control protocol parsing, receiver detection, and transport settings.
A location provider can also be remote or synthetic.

`Time` separates monotonic deadlines from civil UTC.
UTC observations identify the source, validity, and uncertainty.
Shared policy decides whether to accept a correction.
Unknown calendar time is not Unix epoch zero.
Persistent timekeeping and alarms are optional capabilities.
They must specify resolution, range, and lifecycle support.

Boot must not reset persistent time to the build date.
Simulator models must include time retention, loss, and drift where applicable.
UTC corrections must not change the active application's monotonic schedule.
