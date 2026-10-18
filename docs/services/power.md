# Power, charging, and suspend

[Device services](../device-interfaces.md)

System power management and these lifecycle services stay planned work.

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
