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
