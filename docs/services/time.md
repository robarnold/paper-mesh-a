# Calendar time and timers

[Device services](../device-interfaces.md)

## Clock interface

`Clock::now_ms()` supplies monotonic milliseconds for deadlines and observation age.
`Clock::now_utc()` supplies `Option<UtcDateTime>` for calendar timestamps.
Both methods return without I/O, a wait, or an allocation.
An absent UTC value means unknown or invalid calendar time.
It does not mean Unix epoch zero.

UTC corrections can move calendar time forward or backward.
They must not change monotonic deadlines, debounce intervals, or display completion.
Applications must use `now_ms()` for these operations.
The firmware heartbeat and simulator state API are current UTC consumers.
The application screens continue to show receiver observations independently of the clock.

The platform exports calendar types from the `time` crate without its default features.
`UtcReference` combines a complete UTC timestamp with its monotonic receipt time.
Its `at(now_ms)` method adds whole elapsed seconds and handles calendar rollover.
Future receipt times and calendar overflow return no value.
The caller selects validity limits and correction policy.

## Calendar sources

A location report can supply a date and time from the same observation.
`UtcObservation::reference()` returns no reference when the date is absent.
A backend must not combine an old date with a new time of day.
Such a combination can produce the wrong day at midnight.

The firmware uses GNSS reports to set a persistent RTC.
It reads retained RTC time at startup without a GPS fix.
The [PCF8563 adapter](../adapters/rtc.md) specifies device access, calendar validity, and supported range.
The [board RTC profile](../boards/lilygo-t5-s3-pro-rtc.md) records physical integration and validation limits.
The simulator uses deterministic GPS observations and virtual elapsed time.
Neither backend uses the host date or build date as a correction.

These sources supply whole seconds without a measured accuracy bound.
Receipt time includes unknown receiver and transport delay.
The service does not support leap-second values, local time zones, or daylight-saving rules.
Persistent alarms, sleep accounting, drift calibration, and source uncertainty remain planned work.

## Calendar synchronization

The platform [synchronization service](../../crates/platform/src/rtc/synchronization.rs) uses the [`Rtc` contract](../../crates/platform/src/rtc.rs).
The contract separates retained calendar operations from cached `Clock` reads.
It reports access failures, invalid calendar state, unsupported dates, and write verification failures as operation errors.
Invalid time distinguishes voltage loss, a stopped clock, test mode, an unsupported calendar convention, and invalid calendar fields.

The service preserves valid retained time at startup without a location fix.
It advances cached UTC between polls and limits extrapolation after communication stops.
A fresh location observation must supply both date and time before it can correct the RTC.
The service accounts for elapsed time since receipt and consumes each successful correction once.
The first correction sets the calendar. Later corrections use the caller's drift threshold.

Invalid retained time can recover from a fresh source in the same poll.
An access failure immediately invalidates cached UTC and leaves the source observation available for a later retry.
The RTC implementation verifies writes using device readback and documents its tolerance for counter advancement.
A failed verification invalidates cached UTC and leaves the source observation available for retry.
After a verified write, the service reads calendar time again for its cached sample.

The caller supplies the polling interval, observation age limits, correction threshold, and supported years through `Policy`.
The service supplies the next monotonic polling deadline, including after access failures.
Late polls schedule one future poll without a burst of catch-up work.
The [firmware worker](../../firmware/src/rtc/mod.rs) supplies the existing configuration and waits until that deadline through Embassy.
The service does not own a bus or select an executor.
The correction threshold does not establish an accuracy bound.

The firmware clock implements the [suspend participant contract](power.md#suspend-participants).
Preparation invalidates cached UTC after the RTC worker stops.
Wake and canceled preparation leave UTC unavailable until a new RTC read completes.
Cache invalidation makes polling due immediately and preserves consumed correction identities in the existing monotonic epoch.
It does not stop or reset the retained RTC.
The clock also implements the separate [power-off participant contract](power.md#power-off-participants).
It waits for pending worker operations before power removal. An aborted shutdown uses reset and normal clock startup.

## Deterministic verification

Native tests cover invalid dates, midnight, leap days, year rollover, stale reports, retained time, voltage loss, bus failures, and recovery.
They also verify cache invalidation, immediate polling, and correction history across service interruptions.
Simulator tests verify that UTC corrections do not change monotonic time, heartbeats, or completed display frames.
These checks do not establish oscillator accuracy or physical retention.
