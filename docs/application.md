# Application

The application selects workflows and decides when service observations are useful to the reader.
The same [application](../crates/app/src/lib.rs) runs on hardware and in the simulator.
Backends supply observations and logical controls through the [device services](device-interfaces.md).

## Layout and input

Layout and hit testing use the same logical viewport so a mounting change can rotate drawing and touch together.
The application selects the viewport and supplies page geometry. See the [application UI rules](application-ui.md#portrait-layout).

## Deadlines and display submission

The runtime can wait until input arrives, a service changes, or an application deadline expires.
Expiry is a deadline because a previously valid reading can become stale even when its source stays silent.
The application requests another frame for visible changes and uses the [toolkit display contract](platform/toolkit.md#time-and-display-helpers) for submission and retry.

## USB touchpad

In touchpad mode the application gives the host raw contacts so the host can interpret gestures.
The status screen can stay still while fingers move. It needs a redraw when the host status changes.
See the [host session contract](services/touchpad.md) for contact handling across entry, exit, and reconnect.

## Radio debugger

The debugger scans for nearby devices and networks. Changing radios or leaving the debugger cancels the active request.
A late result from that request must never populate a newer scan. Starting a scan clears the previous results.
An empty completed scan and a timeout have different meanings. Keep errors visible until the next request.
See the [debugger](../crates/app/src/debugger.rs) for presentation and the [discovery contract](services/connectivity.md) for result identity and order.

## Location screen

A stale fix means the last position observation is too old to use. An unavailable provider cannot supply a fix.
Both conditions hide coordinates. A receiver can still show activity when it sends messages without a fix.
Position, motion, UTC, receiver activity, and satellite reports expire independently, so a fresh message cannot renew an unrelated reading.

Missing elevation leaves valid horizontal coordinates available. Course describes direction of travel and has no meaning when stopped.
The UTC readout shows the last reported time. It can expire even while the system calendar continues to advance.

The screen paces routine measurement changes to limit e-paper updates. Changes in validity or availability deserve immediate feedback.
See the [location screen and renderer](../crates/app/src/location.rs) for readouts and deadlines, and the [location contract](services/location.md) for observation semantics.

## Power screen

Battery and charger observations come from independent sources. When one fails or expires, its readings disappear and the other remains useful.
The page paces routine measurements but requests a frame promptly for supply changes and faults.
See the [power renderer](../crates/app/src/power.rs) for tab contents and the [power contract](services/power.md#current-observations) for measurement validity.

### Suspend experiment

Use suspend to compare gauge estimates across a controlled sleep interval.
An interrupt can wake the device through incidental contact. Its reported source cannot establish a deliberate gesture.
The report remains available after wake so the reader can inspect elapsed time and signed charge loss.
See the [suspend contract](services/power.md#manual-suspend-experiment) for measurement limits and wake handling.

### Power off

The page requires an explicit shutdown request. A backend-selected held control also permits shutdown when the display cannot update.
The final image can persist after power removal, so a visible request cannot confirm physical shutdown.
See the [power-off contract](services/power.md#explicit-power-off) for prerequisites and failures.

## Diagnostics

Diagnostics groups component checks that run through the same services and renderer as ordinary pages.
See the [menu renderer](../crates/app/src/diagnostics.rs) for the available checks and the [application navigation rules](application-ui.md#destinations-and-operation-restrictions) for exit restrictions.

### Display check

Each solid pattern stays visible until the user advances or exits, so the panel can be inspected without heartbeat redraws.
Advancing requires a fresh press after refresh completion. An early press could otherwise skip the image under inspection.
See the [board procedure](boards/lilygo-t5-s3-pro-display.md#basic-display-check) for physical controls and inspection limits.

### Touch check

The counter retains completed taps across navigation and suspend so a return to the page preserves the result.

### Keyboard check

Each visit starts with an empty field. The [diagnostic page](../crates/app/src/keyboard.rs) supplies placement and capacity
and passes emitted input to the field. The [text input contract](platform/text-input.md) defines editing and layout changes.
