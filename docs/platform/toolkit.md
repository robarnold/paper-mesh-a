# Application toolkit

[Platform overview](../platform.md)

This document describes the implemented toolkit components.

Common functionality belongs in the platform crate.
Reusable components accept caller choices through parameters and callbacks.
They have no dependency on application screens or state.
Toolkit modules group related functions by responsibility.

| Module | Current functionality | Caller choices |
| --- | --- | --- |
| [`ui`](../../crates/platform/src/ui/mod.rs) | Grayscale button rendering | Bounds, label, selection, and enabled state |
| [`ui::format`](../../crates/platform/src/ui/format.rs) | Bounded value labels and satellite labels | Values and format functions with explicit units |
| [`input`](../../crates/platform/src/input.rs) | Tap recognition and cancellation | Hit testing, control identity, and resulting action |
| [`time`](../../crates/platform/src/time.rs) | Periodic deadlines and observation age checks | Epoch, interval, current time, and maximum age |
| [`display`](../../crates/platform/src/display.rs) | Frame submission with busy and error checks | Redraw request and rendering callback |
| [`rotation`](../../crates/platform/src/rotation.rs) | Pixel and touch coordinate conversion | Native dimensions |

`TapGesture` returns a control identity after one contact stays on that control through release.
Movement outside the control cancels the tap. Movement back inside does not arm it again.
A second contact cancels the tap until all contacts lift.
Raw contact events remain available to other consumers.
Release and cancellation clear the armed control.
The application maps the returned identity to an action.
The button renderer uses no fixed screen position or action binding.

`PeriodicTimer` keeps its original phase after a late poll.
It reports at most one event per poll.
Its interval must be greater than zero.

`ObservationWindow` rejects future timestamps and observations at or above the maximum age.
It also calculates the next expiry from supplied receipt times.
Deadline addition saturates at the clock limit.

The formatters use bounded strings without heap allocation.
Function names specify display units, such as `speed_kmh` and `elevation_meters`.
Missing values produce unavailable markers.
The application selects a formatter and decides when a value is usable.

`display::present` renders only when the caller requests a frame and the display is ready.
It reports success after refresh submission.
The caller keeps its redraw request after an error or a deferred submission.
The backend continues to control physical completion and driver recovery.
