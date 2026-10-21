# Simulator scenarios

[Simulator overview](../simulator.md)

These scenarios supply test observations to the current application.
Their delays do not represent measured hardware performance.

## Radio debugger

1. Select `Radio debugger` on the device screen or in the browser controls.
2. Select `Wi-Fi` or `Bluetooth LE`.
3. If virtual time is paused, advance it by two seconds.
4. Use `Next` and `Previous` to see more results.
5. To repeat the scan, select `Scan again`.
6. To return to the counter, select `Back`.

The [application](../application.md#radio-debugger) specifies scan controls and display behavior.
The [discovery contract](../services/connectivity.md#current-discovery-interface) specifies identities and result order.

The `Radio scenario` selector changes the next scan.
The nearby scenario has seven entries for each radio, including an empty name and unknown RSSI.
Other scenarios produce empty results, a failure, an unavailable adapter, or a timeout.
Reset restores the nearby scenario and the counter screen.
The simulator does not access host radios or physical device radios.

## GNSS screen

1. Select `GNSS` on the device screen or in the browser controls.
2. Select a GNSS scenario in the browser.
3. If virtual time is paused, advance it with the time controls.
4. To return to the home screen, select `Back`.

The [location screen](../application.md#location-screen) specifies readouts, units, and expiry.
The simulator page labels test data outside the application screen.

The default scenario supplies a fix after three virtual seconds.
Later observations arrive at one-second intervals.
The scenario selector takes effect immediately and starts a new scenario interval.
Reset restores the default scenario and the home screen.
These intervals are test conditions, not receiver performance measurements.

| Scenario | Behavior |
| --- | --- |
| `fix` | No fix for three seconds, then a valid fix with eight satellites |
| `acquiring` | Reports continue without a position fix |
| `no_signal` | The receiver reports eight satellites without signal levels or a position fix |
| `messages_only` | Receiver messages continue without fix or satellite reports |
| `stale_satellites` | Satellite reports stop. Fix reports and receiver messages continue. |
| `lost_fix` | A valid fix becomes invalid after three seconds |
| `silent` | One valid fix, then no new observations. Application age limits control expiry. |
| `unavailable` | No receiver data |
| `fault` | Receiver fault without coordinates |
| `zero_coordinates` | Valid latitude and longitude of zero |
| `below_sea_level` | A valid position with elevation at -430.5 m above mean sea level |
| `missing_elevation` | Valid horizontal coordinates without an elevation value |
| `stopped` | Zero speed with a valid fix and UTC. The screen hides course. |
| `missing_motion` | Valid position and UTC without speed or course |
| `missing_utc` | Valid position and motion without UTC |
| `stale_motion` | Motion reports stop. Position, UTC, and receiver messages continue. |
| `stale_utc` | UTC reports stop. Position, motion, and receiver messages continue. |

Native tests cover acquisition, fix loss, expiry, faults, recovery, zero coordinates, navigation, and updates during display refresh.
They also cover receiver activity, signal levels without a fix, independent satellite expiry, and message counts.
Elevation tests cover signed values, zero, missing values, fix loss, expiry, and invalid units.
Motion and UTC tests cover unit conversion, direction limits, stops, midnight, invalid fields, fix loss, faults, and independent expiry.

Large and small time advances produce the same location state and completed frame.
Separate native tests execute the firmware decoder and baud selection code.
The simulator does not model satellite reception or verify the physical receiver.

## Calendar clock

Complete GPS reports set the simulated clock from a fixed date of 2020-04-25 and the scenario time of day.
Virtual elapsed time advances the calendar after receiver loss.
No host clock supplies these values.
`Simulation::set_utc` can supply a retained date, inject a correction, or invalidate calendar time with `None`.
Fresh GPS observations can set the clock again.

Native tests cover calendar corrections and loss without changes to monotonic deadlines or display completion.
Firmware tests use simulated I2C registers for retention, voltage loss, malformed values, drift corrections, and bus errors.
The browser does not expose RTC fault controls or model oscillator drift.

## Power screen

Select `Power` on the device screen or in the browser controls.
Select a power scenario in the browser.
The selection takes effect immediately. Reset restores battery discharge and the home screen.
Observations arrive at two-second virtual intervals. Values are test data, not an electrical model.
The [application](../application.md#power-screen) defines display intervals and data expiry.

The Suspend page uses the same application controls as firmware.
The simulator waits for the final display frame, then stops application polls until a wake event ends suspend.
BOOT, Function, and the timer can end suspend. Touch and Home can also end suspend when touch wake is enabled.
The simulator consumes the wake contact through its release.
It retains the counter, screen state, and clock epoch.
Capacity values remain scenario data. The simulator does not model electrical sleep consumption.
Tests cover each wake input, held controls, contact suppression, repeated cycles, charge gain, missing readings, and consistent virtual time steps.
The firmware decoder tests also cover simultaneous events and unattributed expander interrupts.

| Scenario | Behavior |
| --- | --- |
| `battery` | 68 percent, negative battery current, no external input |
| `charging` | USB power with positive current and fast charge |
| `full` | Charge complete with 100 percent and zero current |
| `low_battery` | Four percent and an observed shutdown request without a lifecycle action |
| `no_battery` | USB power with no battery measurements |
| `initializing` | Battery presence without a ready gauge or measurements |
| `gauge_fault` | Gauge read failure with charger data still available |
| `charger_fault` | Charger read failure with battery data still available |
| `temperature_fault` | Charge inhibition, thermal shutdown, and a hot thermistor |
| `unavailable` | Both sources unavailable |
| `stale` | One observation, then no reports. Application age limits control expiry. |

Native tests cover independent failures, recovery, expiry, navigation, and display updates.
Large and small time advances produce the same state and completed frame.
Separate native tests execute the firmware register adapter with simulated I2C responses.

## USB touchpad

1. Select `USB touchpad` on the device screen or in the browser controls.
2. Drag on the display or mini touchpad.
3. Use multiple fingers, or hold Shift during a mouse click to retain a contact.
4. Inspect the contact IDs on the mini touchpad.
5. Press Escape to cancel all retained contacts.
6. Select a USB host state to exercise disconnection, suspend, or faults.
7. Press Function or Home to exit.

The mini touchpad supports up to five concurrent contacts.
Movement changes contact observations and accepted report counts without an e-paper refresh.
A host state change suppresses held contacts until release.
Exit increments the detachment count. The browser cancels any retained synthetic contacts.
Native tests cover activation tap suppression, independent releases, mode exit, cancellation, and reconnects.
USB packet timing and physical gestures require hardware checks.
