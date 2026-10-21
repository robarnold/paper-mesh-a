# Power, charging, and suspend

[Device services](../device-interfaces.md)

Power observations and a manual suspend experiment are implemented.
Charge control and general lifecycle planning stay planned work.

## Current observations

`Power::snapshot` returns cached battery and charger observations without I/O or a wait.
Each source has separate availability, optional data, and an observation time in the monotonic epoch.
Availability distinguishes startup, ready, unavailable, and fault states.
A failed read removes the affected observation. It does not invalidate an independent source.
Consumers select age limits. A snapshot read does not change observation times.

`BatteryState` supplies presence, gauge readiness, optional measurements, and status flags.
Voltage uses mV. Current uses mA. Average power uses mW.
Positive current and power enter the battery. Negative values indicate discharge.
Capacity uses mAh. Percentage values range from zero through 100.
Time estimates use minutes. Missing estimates remain absent.
Gauge temperature describes the gauge silicon in hundredths of a degree Celsius.
It does not describe cell temperature.

The backend omits measurements without a present battery and a ready gauge.
Gauge readiness does not establish correct cell parameters or calibration.
`estimates_verified` requires backend evidence for both conditions.
Unverified estimates retain their values and their qualification.
Battery flags report full charge, full discharge, discharge activity, charge inhibition, overtemperature, and a shutdown request.
These observations do not cause a shutdown.

`ChargerState` supplies the input class, charge phase, configured limits, regulation status, and faults.
The source field distinguishes no input, USB class, adapter class, OTG output, and an unknown class.
USB and adapter classes do not identify the physical connection or distinguish wired power from wireless power.
The input class does not establish a USB data session. OTG output is not external input power.
The charge phase distinguishes idle, precharge, fast charge, and completion.
Configured limits do not establish actual current or effective electrical limits.
Current faults and latched faults have separate fields.
Latched faults describe the interval since the previous device read, not a persistent event log.

The [application power screen](../application.md#power-screen) consumes this interface.
The [hardware adapter](../boards/lilygo-t5-s3-pro-power.md) and [simulator scenarios](../simulator/scenarios.md#power-screen) supply observations.

## Manual suspend experiment

`SuspendRequest` specifies a maximum duration in milliseconds and whether touch interrupts can end suspend.
The backend provides readable wake controls and a timer fallback.
It rejects an unavailable requested input or an input that is already active.
The application requests the transition. The runtime waits for the final display update before preparation.
Preparation failure returns an explicit error and resumes the paused services.

`SuspendReport` records elapsed monotonic time, observed wake events, and fresh power observations before and after the suspend interval.
Control events carry portable control identities. A touch interrupt does not establish a deliberate tap.
An unidentified input remains explicit. The report can contain simultaneous events, and array order does not establish event order.
RAM retains the application and report across light sleep. The experiment does not write the external RTC or flash.
A reset discards the report. The next experiment replaces it.

Charge loss equals the earlier remaining capacity minus the later remaining capacity, in mAh.
Negative loss indicates net charge gain. Missing, invalid, or repeated observations do not produce a comparison.
The current gauge reports capacity in whole mAh. Zero change does not establish zero consumption.
Capacity corrections, temperature, and unverified cell parameters can affect the result.
The report does not claim calibrated energy use or average sleep current.

External power status describes the two endpoints only.
It cannot exclude supply insertion or removal during the interval.
Battery charge change does not measure system consumption when an external supply provides power.
The samples bracket the suspended interval after preparation and before service restoration.
They exclude most display preparation and restoration costs.
Elapsed time describes the interval with sleep permitted, not verified hardware sleep residency.

The [hardware adapter](../boards/lilygo-t5-s3-pro-power.md#manual-light-sleep) defines preparation and verification limits.

## Planned control and lifecycle services

Power status contains supported, valid observations.
Battery percentage is optional.
A backend can report external supply, battery presence, charge state, faults,
voltage, current, temperature, and capacity.
Signed current must use a documented sign convention.
Temperature must have an identified source.

Unavailable, stale, and uncalibrated values must keep those states.
A device can have no battery or controllable charger.
External power and a host data session are independent.
The backend defines cell and electrical limits.
Shared charge policy requests supported operations within those limits.
UI components do not select raw voltages.

Shared policy requests portable lifecycle outcomes:

| Outcome | Portable meaning |
| --- | --- |
| Active idle / service standby | The application continues execution. Inactive services can reduce resource use. |
| Suspend with resume | Resume the same application epoch. Include elapsed time. |
| Suspend with restart | Recover committed or retained state in a new application epoch. |
| Power off | Request supported power removal after preparation. External supply can prevent removal. |
| Forced restart / power loss | Preparation is not guaranteed. Recovery must handle interrupted work. |

The backend maps these outcomes to supported mechanisms.
Requests specify logical wake events, required retained work, and deadlines.
A wake event can identify a control, alarm, or service event.
The backend checks combinations and returns an accepted plan or an explicit
unsupported, busy, or fault result.

Electrical polarity, interrupt routing, and power requirements stay in the
backend. Shared policy must not silently discard a requested wake source.
It must not silently replace resume with restart.

A planned graceful transition has these steps:

1. Stop acceptance of work that cannot finish.
2. Cancel pending requests or wait for their completion under explicit limits.
3. Synchronize durable state.
4. Stop storage operations.
5. Wait for a safe display boundary within a bounded time.
6. Check the requested wake events, including controls that are already active.
7. Arm the wake events without loss of pending events during preparation.
8. Let the backend stop dependencies in order, with resources kept for accepted work and wake events.
9. Enter the selected outcome.

A final e-paper message is optional.
It must not prevent shutdown indefinitely.
If preparation fails, the service reports the cause.
It resumes operation or applies an explicitly selected alternative.

Request acceptance does not prove power removal.
The application might never receive completion after power removal.
The simulator can record the transition externally.
Changed power conditions make a new check of the plan necessary.
Recovery must work without successful shutdown callbacks.
