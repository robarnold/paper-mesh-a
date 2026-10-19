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
The [RTC adapter](../boards/lilygo-t5-s3-pro-rtc.md) specifies synchronization, validity, recovery, and range.
The simulator uses deterministic GPS observations and virtual elapsed time.
Neither backend uses the host date or build date as a correction.

These sources supply whole seconds without a measured accuracy bound.
Receipt time includes unknown receiver and transport delay.
The service does not support leap-second values, local time zones, or daylight-saving rules.
Persistent alarms, sleep accounting, drift calibration, and source uncertainty remain planned work.

## Deterministic verification

Native tests cover invalid dates, midnight, leap days, year rollover, stale reports, retained time, voltage loss, bus failures, and recovery.
Simulator tests verify that UTC corrections do not change monotonic time, heartbeats, or completed display frames.
These checks do not establish oscillator accuracy or physical retention.
