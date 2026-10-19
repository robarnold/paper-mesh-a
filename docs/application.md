# Application

[`paper-mesh-app`](../crates/app/src/lib.rs) contains the current application.
It selects screens, state, touch targets, layout, actions, and application deadlines.
It combines reusable components from `paper-mesh-platform`.
It does not select a board, driver, executor, or CPU core.

The [architecture](architecture.md) shows code dependencies.
The [platform](platform.md) controls resources and execution.
The [device services](device-interfaces.md) specify observation formats and request behavior.

## Layout and input

The current application requires a 540 × 960 logical viewport.
`paper_mesh_app::SCREEN_SIZE` records this layout requirement.
The application uses logical coordinates for rendering and touch targets.
It does not select native panel dimensions or a mounting transform.
Configurable layout and viewport changes stay planned work.

The home screen contains a tap counter, uptime, a grayscale sample, and three navigation buttons.
The platform supplies normalized events.

The application uses the [toolkit gesture](platform/toolkit.md) for tap recognition and cancellation.
It supplies hit testing and action bindings.
All screens use the same button renderer.

## Deadlines and display submission

`App::tick` updates the uptime at 30-second intervals.
The [toolkit timer](platform/toolkit.md) controls late polls and deadline calculation.
The application selects the 30-second interval.
`App::next_deadline_ms` includes the heartbeat, scan timeout, observation expiry, and display intervals.
The runtime waits for these deadlines or earlier service events.

`App::present` renders when application state changes and the display is ready.
It supplies its renderer and redraw request to `display::present`.
Successful submission clears the dirty flag.
Changes during a refresh combine into the next frame.
A drawing error or submission error keeps the frame pending for a retry.
The platform controls completion, driver recovery, and retry delays.

## Radio debugger

The application controls radio selection, scan requests, pagination, and rendering.
The debugger does not connect to networks or pair with devices.
The simulator uses test data and shows `SIMULATED DATA` on the device screen.

The [discovery contract](services/connectivity.md#current-discovery-interface) specifies result order, identity, and capacity.

The debugger shows five entries per page.
An empty name appears as `Hidden network` or `Unnamed device`.
The screen shows the scan time and the result limit when overflow occurs.

The platform calls `App::poll_discovery` for requests, completions, and deadlines.
Each scan has an ID, a radio kind, and a deadline in the application epoch.
The application rejects a completion for a different request.
It cancels a scan after 10 seconds without a valid completion.
The next application deadline includes this timeout.
This timeout belongs to the debugger.

A tab change or exit cancels the active scan.
A new scan clears previous results and resets pagination.
Scan errors stay visible until another request.
The [simulator guide](simulator/scenarios.md#radio-debugger) specifies scan scenarios and artificial delays.

## Location screen

The application controls the GNSS screen, touch targets, observation validity, and display deadlines.
The home screen has a `GNSS` button. The location screen has a `Back` button.
`App::poll_location` reads a `LocationSnapshot` from the service.
The screen does not identify hardware or simulator providers.
The simulator page labels test data outside the application screen.

The screen shows elevation in meters, rounded to one decimal place, with the reference `MSL`.
Elevation requires a fresh, valid position fix. Missing elevation does not invalidate horizontal coordinates.

The screen shows speed in km/h and course in degrees, each rounded to one decimal place.
Course also has an eight-point compass direction. The screen shows course only when reported speed is greater than zero.
Course describes direction of travel. It does not indicate device orientation or guarantee a reliable direction at low speed.

The screen shows the last reported time as `HH:MM:SS`. Midnight is valid.
UTC and motion have independent validity and receipt times.
Neither requires a position observation in application policy.
The clock readout does not extrapolate time, supply a date, or change the monotonic clock.
It does not set the RTC or establish clock accuracy.

The screen shows coordinates only for a ready provider with an observation less than five seconds old.
It rejects observation times after the current application time.
Expired data, faults, and unavailable data remove the affected readouts from the screen.
The same five-second age limit applies independently to position, motion, UTC, receiver activity, and satellite data.
Observation age stays visible for expired data.
`ObservationWindow` supplies age checks and expiry calculations with the limit from the application.

The screen updates values at five-second intervals to limit e-paper refreshes.
Fix status, receiver status, and changes in readout availability update immediately.
Display changes combine while a refresh is pending.

A recent valid message makes the receiver active, including messages without a position fix.
After five seconds without a valid message, the screen shows `No recent response`.
This status proves recent receiver output. It does not prove a satellite fix or command acknowledgement.

The satellite readout shows counts and the strongest reported signals.
The toolkit supplies common value formatters and short labels for satellite systems.
The screen expires satellite data independently of fix data and receiver activity.
If storage capacity is insufficient, `truncated` marks the counts as lower bounds. The screen adds `+` to these counts.
The [location contract](services/location.md) specifies units, validity fields, and receipt times.

The application includes position, motion, UTC, activity, and satellite expiry in `App::next_deadline_ms()`.
The backend notifies the platform loop when its latest observation changes.
The location screen does not cause periodic display refreshes while another screen is open.
The [simulator guide](simulator/scenarios.md#gnss-screen) defines deterministic location scenarios.
UTC date, clock synchronization, accuracy estimates, and acquisition requests stay planned work.

## Power screen

Select `Power` on the home screen to open the battery details.
Select `Battery` or `Charger` to change the detail tab.
Select `Back` to return to the home screen.

The battery tab shows charge percentage, a capacity bar, voltage, instantaneous current, average current, and average power.
It also shows gauge temperature, capacity estimates, health percentage, cycles, time estimates, readiness, and battery flags.
Positive current enters the battery. Gauge temperature does not measure cell temperature.
The screen identifies unverified cell parameters and calibration.

The charger tab shows supply type, charge phase, power status, configured limits, and regulation status.
A fault table separates current faults from latched faults.
Neither tab permits changes to charger settings or system power.
The [power contract](services/power.md#current-observations) specifies units and validity.

`App::poll_power` accepts independent battery and charger observations.
The screen removes values after a read fault, an unavailable source, or an observation age of ten seconds.
Future observation times supply no valid values. Missing values appear as `--`.
Each tab shows data availability and age for both sources.

Measurement changes update at five-second intervals.
Supply changes, charge changes, faults, and validity changes update immediately.
Display changes combine while a refresh is pending.
`App::next_deadline_ms` includes power expiry and display deadlines only while the power screen is open.
The [simulator scenarios](simulator/scenarios.md#power-screen) cover supply changes, missing data, failures, and recovery.
