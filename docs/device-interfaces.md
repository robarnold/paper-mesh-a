# Portable device services

This proposed service design has a revision date of 2026-10-13.
Names identify intended contracts, not implemented Rust APIs.
[architecture.md](architecture.md#boundaries) lists the current interfaces.
[Backend profiles](hardware.md) contain physical inventories and chip mappings.
A backend can omit services from this catalog.

## Ownership and common contract

Application components use input, location, files, time, power status, lighting,
and communications. Shared policy controls gestures, data freshness, request
scheduling, persistence, and lifecycle requests.
Backend adapters supply observations and execute accepted operations.

A backend resource coordinator controls buses, interrupts, power dependencies,
driver initialization, and recovery.
It controls access to resources that multiple services share.
Application code operates without wiring details.
The coordinator is internal to the backend.
It is not an application device service.

Each service contract must specify these elements:

| Contract element | Required meaning |
| --- | --- |
| Capabilities | Supported operations and limits. Separate absent hardware, unsupported operations, and missing adapters. |
| Availability | Uninitialized, starting, ready, suspended/off, or faulted. A service can define more detailed states. |
| Observations | Values with units, validity, and monotonic observation time. Missing or stale data is not zero. |
| Requests | Bounded work with identity, necessary deadlines, acceptance or rejection, and completion or error results. |
| Buffer ownership | Copy or borrow behavior, required lifetime, and return of ownership after cancellation. |
| Reset and persistence | State that survives resume, restart, power loss, or media replacement. Old completions cannot satisfy new requests. |
| Backpressure | Bounded queues, rules to combine updates, required event delivery, and overflow reports. |

A board profile stays private to its backend.
It gives information about wiring, fitted devices, mounting, calibration, and dependencies.
The backend supplies capabilities, logical control descriptors, display geometry,
and lifecycle outcomes. Application policy must not depend on board IDs, chip
names, GPIOs, or rail connections.

Capability selection can occur at build time or startup.
It uses no dynamic loading, heap allocation, or universal driver registry.
Capabilities and current readiness are separate.
A missing adapter cannot supply a working service.
A location provider that searches for a fix is present but has no current fix.

Components declare their requirements.
They explicitly handle unavailable optional services.
Lighting controls depend on lighting capability.
Automatic rotation depends on orientation observations.
A stale location must not appear as a current fix.
Capability changes invalidate dependent work and UI controls.

Asynchronous requests must have a boot or session identity, monotonic timestamps, and
request IDs. Suspend with resume keeps the application epoch and includes
elapsed time. Restart creates a new epoch.
Wall-clock corrections must not move debounce, timeout, or display deadlines
backward. Queue overflow must invalidate affected input state.
It must not silently discard a release event.

## Service catalog

| ID | Service / consumers | Planned operations and observations |
| --- | --- | --- |
| DEV-INPUT | Input → navigation and interactions | Contacts, control edges or activations, cancellation, resynchronization, optional pen reports |
| DEV-DISPLAY | Display → compositor | Geometry, damage, modes, capabilities, identified submissions and completions, presentation faults |
| DEV-LIGHT | Display lighting → reading UI and power policy | Brightness and off requests, applied level, availability |
| DEV-LOCATION | Location → navigation, timestamps, messaging policy | Optional acquisition requests, provider state, fix, validity, age |
| DEV-FILES | Files → content, tracks, logs | Media discovery and mount, bounded reads and writes, sync, close, unmount, media generation, capacity, errors |
| DEV-SETTINGS | Durable internal state → preferences and recovery | Versioned settings, atomic commit or explicit failure, recovery of a committed version |
| DEV-TIME | Time → timers, timestamps, wake planner | Monotonic clock, optional UTC observations, time correction policy, persistent alarm capability |
| DEV-POWER | Power and lifecycle → all services | Valid measurements, bounded charge requests, suspend and off outcomes, wake plans, boot report |
| DEV-RADIO | Packet radio → shared communications protocol | Supported PHY configuration, receive, transmit, cancel, standby, identified results, link metadata |
| DEV-CONNECTIVITY | Optional host and network transports → shared services | Discovery scans, session state, bounded transfers, disconnect, diagnostics |
| DEV-ORIENTATION | Orientation policy → layout, input, output | Manual orientation and lock, optional orientation observations |

These are logical boundaries.
Related services can share an implementation.
A service does not correspond to one trait per chip.
For example, a wall-clock observation can be independent of a location fix.
Neither service depends on a particular clock chip or receiver.

Packet-radio consumers operate below application messaging.
UI components can operate without physical radio configuration.

The implemented interfaces cover the clock, single-contact touch, display
submission, busy state, fixed rotation, and discovery scans.
Capability descriptors and other catalog entries stay planned work.
Backend profiles and the [simulator guide](simulator.md) record implementation
status.

## Input and lifecycle controls

A backend supplies stable control identities, labels, and supported event kinds.
An identity is separate from an action binding such as back, select, or sleep.
Profiles can supply default bindings.
Shared policy controls remapping and gestures.
Display rotation does not change a control's identity.
The design uses no universal list of enclosure buttons.

- Controls with edge reports supply debounced `Pressed` and `Released`
  observations with timestamps. Shared policy derives each gesture once.
  Startup synchronization detects held controls without a incorrect click on release.
- Controls with activation reports supply `Activated`.
  An activation callback cannot establish accurate hold or repeat behavior.
  A separate capacitive key produces a control event.
  It does not produce an invented display coordinate.
- Input loss, reset, driver restart, or overflow cancels armed gestures.
  An orientation transition cancels gestures that depend on coordinates.
  Button state stays independent of pointer coordinates.
- Multiple contacts, pressure, tilt, hover, pen buttons, and eraser state are
  optional capabilities. Their implementations must keep contact and tool
  identity. The current `Down`/`Up` API cannot represent these reports.

Some physical controls operate outside the application.
They can force a reset, remove power, or select a boot mode.
The backend models these lifecycle effects without invented key callbacks.
The application receives only observable boot or reset information.
It receives `Unknown` when the backend cannot identify the cause.

A simulator trace can record an external stimulus that the application cannot
observe. Backend conformance tests give information about particular boot sequences.

## Location and time

`Location` separates provider availability, acquisition state, and fix validity.
Observations can include latitude, longitude, altitude, speed, course, accuracy,
and measurement time. Altitude must have a reference.
Provider metadata is optional.
Each field must have units and validity.
`(0, 0)` is valid data, not a missing-fix marker.

Local receipt time and source measurement time are separate.
A cached observation must not appear as a new fix.
Position validity and UTC validity are independent.
Course during movement does not replace device orientation.

Timing precision depends on the source and synchronization path.
A received message does not necessarily identify a accurate clock edge.
Adapters control protocol parsing, receiver detection, and transport settings.
A location provider can also be remote or synthetic.

`Time` separates monotonic deadlines from civil UTC.
UTC observations identify the source, validity, and uncertainty.
Shared policy decides whether to accept a correction.
Unknown calendar time is not Unix epoch zero.
Persistent timekeeping and alarms are optional capabilities.
They must specify resolution, range, and lifecycle support.

Boot must not reset persistent time to the build date.
Simulator models must include time retention, loss, and drift where applicable.
UTC corrections must not change the active application's monotonic schedule.

## Files and internal persistence

Removable files and internal settings are separate services.
Media removal must not erase settings or prevent application startup.
The backend controls flash partitions, erase geometry, wear management, and
update slots. Volatile memory cannot supply durable settings storage.

`Files` reports media states such as unknown, absent, probing, mounted,
unmounted, read-only, and faulted.
It reports capacity and free space when known.
Presence detection can be immediate or delayed.
Applications use service observations.
Only the backend knows whether detection uses a signal, polling, or an I/O
failure. Simulated media changes must use the declared detection path.

Each mount has a media generation.
Replacement invalidates old handles and completions, even when filenames match.
Reads and writes must bound work and report partial progress.
Errors include full media, unsupported or corrupt filesystems, timeout, removal,
and I/O failure.

Write, sync, close, and unmount must have their own durability guarantees.
Sudden power or media loss can damage filesystem state.
Atomic replacement guarantees depend on a selected and validated storage strategy.

Resource contention must not prevent other services from progress.
Applications can operate without identification of the shared bus.
Unmount and electrical power removal are separate capabilities.
Neither operation implies the other.

`Settings` must recover the last or new committed version after interruption.
It must include schema handling, version handling, capacity results, and explicit errors.
The implementation must meet this guarantee before it reports the capability.
A simulator can keep durable media across an application restart.
It can discard volatile buffers and incomplete writes during that restart.

## Power, charging, and suspend

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

## Lighting, communications, orientation, and resource ownership

`DisplayLighting` accepts normalized brightness and an explicit off value.
It reports the applied level and errors.
Backend configuration defines the electrical range and brightness mapping.
Lighting changes do not submit a panel refresh.
Automatic brightness also depends on an available ambient-light source.
Lighting capability alone does not supply that source.

`PacketRadio` reports supported PHY settings and payload limits.
Received packets include supported signal metadata.
Transmit results identify the request.
Transmit completion does not confirm peer delivery.
Overflow, deadlines, reset, and availability changes must have observable failure
results. Shared software selects mesh protocols independently of the radio driver.

Host and network transports report their capabilities and session state.
Features that depend on a connection stay unavailable without that connection.
File export must control media ownership.
Host and local filesystem writers must not write the same media at the same time.
OTA and update policy stay separate design work.

`Discovery` implements the scan operation for Wi-Fi and Bluetooth LE.
The [radio debugger](architecture.md#radio-debugger) defines its current limits and consumer.
It returns names, addresses, address types, and optional RSSI in dBm.
A completion includes its request and observation time.
The adapter identifies simulated or physical observations when it accepts a request.
Current adapters provide simulated observations, physical observations, and unsupported operation reports.

The adapter writes completed results into storage that the consumer supplies.
It leaves that storage unchanged when no completion is available.
Errors clear that storage.
Cancellation invalidates a request immediately.
The adapter releases resources before completion delivery or the next physical scan.
Resource release can finish asynchronously after cancellation.
It must discard completions from an earlier application session.
The consumer accepts only completions for its active request.
Busy, unsupported, failure, and timeout results are separate errors.
An empty successful scan is not an error.
Connection, pairing, and application data transfer stay outside this operation.

`Orientation` supports manual selection and an optional automatic source.
It reports unavailable sources explicitly.
Shared policy controls sensor filters and logical overrides.
Backends supply native geometry, mounting information, and calibration.
Scenario profiles can include or omit sensors.

Multiple drivers can share a resource, but that resource must have one coordinated
owner. A service release must not invalidate another service's accepted work.
A shared interrupt must dispatch to its actual sources.
Failed reads cannot prove released controls or healthy power.
Recovery restores resource state and informs affected services.
The bus and power topology stays private to the backend.

## Simulator acceptance scenarios

These scenarios stay planned work.
They are not currently successful tests.
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
| Edge control bounce or held control at startup | Debounce produces one gesture. Synchronization prevents a incorrect click. |
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

The current Reset action resets the test fixture.
Lifecycle restart, resume, power loss, and media persistence must have different
controls. [Backend profiles](hardware.md) give information about conformance cases for the
first physical target.

## Decisions to resolve during implementation

- Add capability descriptors, geometry, and control bindings when consumers and
  backends can verify their contracts.
- Select debounce rules and gesture policy.
- Define bounded queues and overflow behavior.
- Select filesystem and settings strategies.
- Verify their durability guarantees.
- Establish service deadlines and cost reports under resource contention.
  The backend selects core placement and electrical budgets.
- Select communications protocols and update policy.
  Each backend can supply a different set of transports.
