# Service validation

[Device services](../device-interfaces.md) · [Simulator](../simulator.md)

These scenarios define planned coverage across multiple profiles.
The [current scenarios](../simulator/scenarios.md) exercise selected cases for implemented interfaces.
They do not establish all of the coverage below.

They use virtual time and explicit stimuli.
Costs stay synthetic until measurements are available.
Scenario configuration and debug traces stay separate from application
capabilities and observations.

Contract tests must use multiple synthetic profiles.
A board preset adds electrical and lifecycle constraints to the same contracts.

| Scenario | Required observable behavior |
| --- | --- |
| Missing optional services | Components use supported alternatives. Unavailable operations do not report success. |
| Different geometry, mounting, and controls | Application policy stays the same. Layout uses the logical viewport. Bindings use declared controls. |
| Edge control bounce or held control at startup | Debounce produces one gesture. Synchronization prevents an incorrect click. |
| Activation-only control or optional pen | Unsupported hold and pen operations stay unavailable. No invented pointer coordinates occur. |
| Restart during a write or refresh | Old requests expire. Data and panel retention obey declared capabilities. |
| Location search, stale fix, or unavailable provider | Availability, field validity, source precision, and age stay correct. |
| One service stops while a different service uses shared resources | The remaining service keeps its accepted work. The application uses no topology details. |
| Media absent, inserted late, removed, or replaced | Detection uses the declared delay. Replacement invalidates old handles and completions. |
| Concurrent storage and radio work | Progress and backpressure are bounded. Neither service waits indefinitely without a report. |
| UTC correction and persistent-clock loss | Monotonic timers stay unchanged. The service reconciles invalid civil time and alarms. |
| Power-source change during shutdown | The backend checks the transition again. It reports no incorrect power-off or charge success. |
| Unsupported wake combination | Rejection keeps the requested wake requirement. |
| Lighting change during display idle | No panel refresh is necessary. |
| Resource failure and recovery | Dependent observations become unavailable or stale. Input and resource state synchronize again. |
| Automatic orientation source present or absent | Automatic mode depends on observations. Manual selection stays possible. |

The [simulator limits](../simulator.md#limits) describe the current Reset action.
Lifecycle restart, resume, power loss, and media persistence must have different
controls. [Backend profiles](../hardware.md) give information about conformance cases for the
first physical target.
