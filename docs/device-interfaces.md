# Portable device services

This document specifies portable service contracts and planned extensions.
[`paper-mesh-platform`](../crates/platform/src/lib.rs) contains the implemented interfaces.
The catalog identifies both current services and planned extensions.
The [architecture](architecture.md#boundaries) shows code dependencies.
[Backend profiles](hardware.md) contain physical inventories and chip mappings.
A backend can omit services from this catalog.

## Implemented interfaces

| Interface | Contract | Backend responsibility |
| --- | --- | --- |
| `Clock::now_ms()` | Monotonic milliseconds from a stable epoch | Keep the epoch across waits. Include elapsed sleep time. |
| `Clock::now_utc()` | Optional current UTC date and time | Validate calendar sources. Preserve monotonic deadlines during corrections. |
| `TouchEvent` | `Down`, `Up`, or `Cancel`, with logical viewport coordinates | Decode contacts. Calibrate axes. Transform coordinates. Deliver events in order. |
| `Display` | `DrawTarget<Color = Gray4>`, `is_busy()`, and `refresh()` | Control storage, pixel packing, submission, completion, and errors. |
| `Discovery` | Identified scans, bounded results, source, receipt time, cancellation, and errors | Supply observations. Control radio resources. Report unsupported operations. |
| `Location` | Position, motion, UTC, receiver activity, satellite signals, availability, and receipt times | Decode observations. Control the transport and supply. Keep original receipt times. |

These interfaces contain no screen layout, fixed panel dimensions, or application timeout.
`PortraitDisplay` and `PortraitRotation` supply coordinate adapters in the same crate.
The [runtime](platform/runtime.md) specifies execution.
The [display lifecycle](platform/display.md) specifies display ownership.
The [application](application.md) specifies consumer behavior and deadlines.

## Ownership and common contract

These requirements guide new contracts and extensions.
The implemented interfaces above do not yet represent every requirement.

Application components use input, location, files, time, power status, lighting,
and communications. Applications select action bindings, observation age limits, and service requests.
The platform toolkit supplies common gesture recognition, timing, and observation checks.
Planned platform policy will control persistence, resource scheduling, and lifecycle execution.
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

| ID | Service / consumers | Contract scope |
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
submission, busy state, fixed rotation, discovery scans, and basic location observations.
Capability descriptors and other catalog entries stay planned work.
Backend profiles and the [simulator guide](simulator.md) record implementation
status.

## Detailed contracts

| Topic | Status and scope |
| --- | --- |
| [Input and lifecycle controls](services/input.md) | Current pointer events and planned control capabilities |
| [Location observations](services/location.md) | Current observations and planned acquisition extensions |
| [Calendar time and timers](services/time.md) | Current UTC clock, monotonic deadlines, and planned alarms |
| [Files and internal persistence](services/storage.md) | Planned media and durable settings contracts |
| [Power, charging, and suspend](services/power.md) | Planned power observations and lifecycle transitions |
| [Discovery and communications](services/connectivity.md) | Current scans and planned packet and transport services |
| [Display contract](display-constraints.md) | Planned display and lighting services |
| [Orientation state](platform/orientation.md) | Fixed coordinate adapters and planned orientation policy |
| [Service validation](platform/service-validation.md) | Planned portable acceptance scenarios |

## Resource ownership

The [orientation design](platform/orientation.md) specifies planned selection policy and current coordinate adapters.

Multiple drivers can share a resource, but that resource must have one coordinated
owner. A service release must not invalidate another service's accepted work.
A shared interrupt must dispatch to its actual sources.
Failed reads cannot prove released controls or healthy power.
Recovery restores resource state and informs affected services.
The bus and power topology stays private to the backend.

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
