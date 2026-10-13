# Portable device services

This is the proposed service design, revised on 2026-10-13. Names describe
intended contracts, not implemented Rust APIs. The current interfaces are listed
in [architecture.md](architecture.md#boundaries). Physical inventories and the
mapping from chips to these services belong to [backend profiles](hardware.md).
A backend need not provide every service in this catalog.

## Ownership and common contract

Application components consume input, location, files, time, power status,
lighting, and communications. Shared policy owns gesture interpretation,
freshness, request scheduling, persistence policy, and lifecycle intent. Backend
adapters provide observations and perform accepted operations.

A backend resource coordinator owns buses, interrupt dispatch, power-domain
dependencies, driver initialization, and recovery. It can arbitrate resources
shared by several services without exposing its wiring to application code.
It is an implementation boundary, not another application-facing device service.

Every service should specify:

| Contract element | Required meaning |
| --- | --- |
| Capabilities | Supported operations and limits; distinguish absent hardware, unsupported operations, and unimplemented adapters |
| Availability | Uninitialized, starting, ready, suspended/off, or faulted for a present service; service-specific substates refine this |
| Observations | Values with units, validity, and monotonic observation time; missing/stale data is not zero |
| Requests | Bounded work with identity, deadlines where needed, acceptance/rejection, and completion/error semantics |
| Buffer ownership | Whether inputs are copied or borrowed, how long they remain valid, and when cancellation returns ownership |
| Reset and persistence | What survives resume, application restart, power loss, or media replacement; old completions cannot satisfy new requests |
| Backpressure | Bounded queues; declare which updates coalesce, which must be delivered, and how overflow is reported |

A board profile is private to its backend: it describes wiring, fitted devices,
mounting, calibration, and dependencies. The backend exposes service capabilities,
logical control descriptors, display geometry, and lifecycle outcomes. The app
must not branch on a board ID, chip name, GPIO, or a rail relationship. Capability
selection may be static at build time or established at startup; this does not
require dynamic loading, heap allocation, or a universal driver registry.

Capabilities and current readiness are different. An expected location provider
with an unimplemented adapter is not a working location service; a provider
searching for a fix is not absent. Components declare their requirements and
handle unavailable optional services explicitly: no lighting control without
lighting, no automatic rotation without orientation observations, and no stale
location silently presented as a current fix. Capability changes invalidate
pending work or UI affordances that relied on them.

Use a boot/session identity with monotonic timestamps and asynchronous request
IDs. A resumable suspend preserves the application's epoch and accounts for
elapsed time; a restart creates a new epoch. Wall-clock corrections must not
move debounce, timeout, or display deadlines backward. Queue overflow must
invalidate affected input state rather than silently losing a release.

## Service catalog

| ID | Service / consumers | Planned operations and observations |
| --- | --- | --- |
| DEV-INPUT | Input → navigation and interactions | Contacts, control edges or activations, cancellation/resynchronization, optional pen reports |
| DEV-DISPLAY | Display → compositor | Geometry, damage, mode/capabilities, identified submission/completion, presentation faults |
| DEV-LIGHT | Display lighting → reading UI and power policy | Brightness/off request; applied level and availability |
| DEV-LOCATION | Location → navigation, timestamps, messaging policy | Request acquisition where supported; observe provider state, fix, validity and age |
| DEV-FILES | Files → content, tracks, logs | Discover/mount media, bounded read/write, sync, close/unmount; media generation, capacity and errors |
| DEV-SETTINGS | Durable internal state → preferences and recovery | Read/update versioned settings, atomic commit or explicit failure, recover a committed version |
| DEV-TIME | Time → timers, timestamps, wake planner | Monotonic clock; optional UTC observations, time-setting policy and persistent alarm capability |
| DEV-POWER | Lifecycle/power → all service lifecycles | Valid power measurements, constrained charging policy, supported suspend/off outcomes and wake plans, boot report |
| DEV-RADIO | Packet radio → shared communications protocol | Supported PHY configuration, receive/transmit, cancel/standby; identified results and link metadata |
| DEV-CONNECTIVITY | Optional host/network transports → shared services | Session state, bounded transfers, disconnect and diagnostics |
| DEV-ORIENTATION | Orientation policy → layout/input/output | Manual orientation/lock and optional orientation observations |

These are logical boundaries; related services can share one implementation.
They are not one trait per chip. A wall-clock observation, for example, can be
provided independently of a location fix without requiring a particular clock
chip or receiver. Packet-radio consumers belong below application messaging;
UI components need not configure a physical radio themselves.

The implemented baseline is still clock, single-contact touch, display
submission/busy state, and a fixed rotation adapter. Capability descriptors and
the other catalog entries remain planned. Backend-specific progress is recorded
in the profiles and [simulator guide](simulator.md).

## Input and lifecycle controls

A backend exposes stable control identities and descriptors with labels and
supported event kinds. Logical identities are distinct from action bindings
such as back, select, or sleep. Profiles can supply default bindings, while
shared policy owns remapping and gestures. A control's identity does not change
when the display rotates. No universal list of enclosure buttons is required.

- Edge-capable controls provide debounced `Pressed` and `Released` observations
  with timestamps. Shared policy derives gestures once. Synchronize held controls
  at startup without creating a phantom click on release.
- Activation-only controls provide `Activated`. Do not derive accurate hold or
  repeat behavior from an activation callback. A separate capacitive key remains
  a control event, not a fabricated coordinate in the display viewport.
- Input loss, reset, driver restart, or overflow cancels armed gestures. An
  orientation transition cancels coordinate-dependent gestures; button state is
  independent of pointer coordinates.
- Multiple contacts, pressure, tilt, hover, pen buttons, and eraser state are
  optional capabilities. Preserve contact/tool identity when implemented; the
  current `Down`/`Up` API does not represent these richer reports.

Some physical controls operate outside the application and can force a reset,
remove power, or select a boot mode. The backend models those lifecycle effects
without inventing application key callbacks. The app receives only observable
boot/reset information, including `Unknown` where the cause cannot be identified.
A simulator trace can know the external stimulus without granting that knowledge
to the application. Particular boot sequences belong to backend conformance tests.

## Location and time

`Location` separates provider availability, acquisition state, and fix validity.
Observations can include latitude/longitude, altitude with its reference,
speed/course, accuracy, and measurement time where available. Provider-specific
metadata is optional. Each field has units and validity; `(0, 0)` is valid data,
not a no-fix sentinel. Record local receipt time as well as source measurement
time, so a cached observation does not silently become a new fix.

Position and UTC validity are independent. Course while moving does not replace
a device-orientation observation. Timing precision must reflect the actual
source and synchronization path. A received message is not inherently a precise
clock edge. Protocol parsing, receiver probing, and transport settings belong to
adapters. A location provider could also be remote or supplied by a scenario.

`Time` separates monotonic deadlines from civil UTC. UTC observations carry source
and validity/uncertainty; shared policy decides whether to accept a correction.
Unknown calendar time is not Unix epoch zero. Persistent timekeeping and alarms
are optional capabilities with explicit resolution, range, and lifecycle support.
Do not reset persistent time to the build date on each boot.

Simulators should model time retention/loss and drift where those capabilities
exist. UTC corrections must not change a running app's monotonic schedule.

## Files and internal persistence

Removable files and internal settings are distinct services. Removing optional
media must not erase settings or prevent the application from starting. Flash
partitions, erase geometry, wear management, and update slots belong to the
backend; volatile working memory is never a durable settings store.

`Files` exposes media states such as unknown, absent, probing, mounted,
unmounted, read-only, and faulted, with capacity/free space where known. Presence
detection can be immediate or delayed. Applications act on service observations;
only the backend knows whether those came from a hardware signal, polling, or
an I/O failure. Simulated insertion/removal must use the declared detection path.

Each mount has a media generation. Replacement invalidates old handles and
completions even when filenames match. Bound reads/writes and report partial
progress, capacity exhaustion, unsupported/corrupt filesystems, timeout,
removal, and I/O failure. State the durability guarantees of write, sync, close,
and unmount separately. Sudden loss can still damage filesystem state; atomic
replacement guarantees depend on a selected and validated storage strategy.

Operations must permit other services to make progress under resource contention.
Applications do not know which bus is shared. Unmounting media and removing its
power are separate capabilities; neither implies the other.

`Settings` should recover either the previous or new committed version after
interruption, with schema/version handling and explicit capacity/error results.
Implement that guarantee before advertising it. A simulator can retain durable
media across an application restart while discarding volatile buffers and
incomplete writes.

## Power, charging, and suspend

Power status consists of supported, valid observations, not a mandatory battery
percentage. A backend may report external supply/battery presence, charge state
and faults, voltage, signed current with a documented convention, temperature
with its source, and capacity. Preserve unavailable, stale, and uncalibrated
values. A device may have no battery or no controllable charger.

External power and a host data session are independent states. The backend owns
cell/electrical constraints; shared charging policy requests only advertised
operations within those constraints. UI components do not select raw voltages.

Shared policy requests lifecycle outcomes rather than processor-specific modes:

| Outcome | Portable meaning |
| --- | --- |
| Active idle / service standby | App keeps running; inactive services can reduce resource use |
| Suspend with resume | Resume the same application epoch and account for elapsed time |
| Suspend with restart | Recover committed/retained state in a new application epoch |
| Power off | Request supported power removal after preparation; external supply may prevent it |
| Forced restart / power loss | No preparation guarantee; recover interrupted work |

The backend maps these outcomes to supported mechanisms. Requests specify
logical wake events (a control, alarm, or service event), required retained work,
and deadlines. It validates combinations and returns an accepted plan or an
explicit unsupported/busy/fault result. Electrical polarity, interrupt routing,
and powered-rail requirements remain inside the backend. Shared policy must not
silently discard a requested wake source or substitute a restart for a resume.

A planned graceful transition is:

1. Stop admitting work that cannot finish; cancel or drain requests explicitly.
2. Sync durable state, quiesce storage, and reach a display-safe boundary. A last
   e-paper message is best effort, not grounds to block shutdown indefinitely.
3. Validate and arm wake events, including already-asserted controls, without
   allowing pending events to be lost during preparation.
4. Let the backend quiesce dependencies in order, retaining resources needed by
   accepted work and the wake plan.
5. Enter the selected outcome; on preparation failure, report why and resume or
   apply an explicitly chosen fallback.

Acceptance is not proof that power disappeared: the app may never observe a
completion after power is removed. The simulator can record that transition
externally. Changing power conditions requires revalidation. Recovery must work
without successful shutdown callbacks.

## Lighting, communications, orientation, and resource ownership

`DisplayLighting` has a normalized brightness request, explicit off value,
applied-state feedback, and errors. Backend configuration owns the electrical
range and brightness mapping. Lighting changes do not submit a panel refresh.
Automatic brightness additionally requires an available ambient-light source;
it cannot be inferred from the presence of lighting.

`PacketRadio` reports supported PHY settings, payload limits, receive packets
with supported signal metadata, and identified transmit results. Transmit
completion does not mean peer delivery. Overflow, deadlines, reset, and
availability changes need observable failure paths. Mesh/protocol choice is a
shared software decision separate from a radio chip driver.

Host/network transports expose their own capabilities and session state.
Connection-dependent features remain unavailable without them. File export must
arbitrate ownership rather than allow host and local filesystem writers to use
the same media concurrently. OTA/update policy is a separate future design.

`Orientation` supports manual selection and an optional automatic source, with
an explicit unavailable state. Sensor filtering and logical overrides are shared
policy; native geometry, mounting, and calibration are backend inputs. Scenario
profiles may exercise either sensor-present or sensor-absent behavior.

A resource shared by multiple drivers has one coordinated owner. Releasing one
service cannot invalidate another service's accepted work. A shared interrupt
must be dispatched to its actual sources. Failed reads do not imply released
controls or healthy power. Recovery restores resource knowledge and notifies
affected services. The board's bus and power graph remains private throughout.

## Simulator acceptance scenarios

These are planned scenarios, not tests that currently pass. Use virtual time and
explicit stimuli, with synthetic costs until measured. Scenario configuration
and debug traces are separate from app-visible capabilities and observations.
Run these contracts across several synthetic profiles; a board-specific preset
adds its electrical/lifecycle constraints to the same contracts.

| Scenario | Required observable behavior |
| --- | --- |
| Missing optional services | Components use supported fallbacks; unavailable operations do not report success |
| Different native geometry, mounting, and control inventory | Same app policy; layout uses the logical viewport and bindings use declared controls |
| Edge control bounce / held at startup | One gesture after debounce; no phantom click after synchronization |
| Activation-only control / optional pen | Unsupported hold/pen operations remain unavailable; no fabricated pointer coordinates |
| Restart during storage write or refresh | Old requests expire; committed data and panel retention follow declared capabilities |
| Location searching, stale, or unavailable | Correct availability, field validity, source precision, and age |
| One service stops while another retains shared resources | Remaining service keeps its accepted work; no topology knowledge in the app |
| Media absent, inserted late, removed, replaced | Detection latency honored; old handles/completions invalidated |
| Concurrent storage and radio work | Bounded progress/backpressure; no silent indefinite starvation |
| UTC correction and persistent-clock loss | Monotonic timers unaffected; invalid civil time and alarms reconciled |
| Power-source change during shutdown | Transition revalidated; no false off/charge-success report |
| Unsupported wake combination | Plan rejected without losing the requested wake requirement |
| Lighting change during display idle | No panel refresh required |
| Resource failure and recovery | Dependent observations become unavailable/stale; input and resource state resynchronize |
| Automatic orientation present versus absent | Auto works only with observations; manual selection remains possible |

The simulator's current Reset action resets the test fixture. Lifecycle restart,
resume, power loss, and media persistence will need separate controls. See
[backend profiles](hardware.md) for the first physical target's conformance cases.

## Decisions to resolve during implementation

- Introduce capability descriptors, geometry, and control bindings when their
  first consumers and backends can validate the contract.
- Select debounce/gesture policy, bounded queues, and overflow behavior.
- Select filesystem/settings strategies and verify advertised durability.
- Establish service deadlines and cost reporting under resource contention;
  core placement and electrical budgets remain backend choices.
- Choose communications protocols and update policy without requiring every
  backend to provide every transport.
