# Runtime execution

[Platform overview](../platform.md)

## Implementation boundaries

[`paper-mesh-platform`](../../crates/platform/src/lib.rs) contains toolkit implementations and portable service contracts.
It uses `no_std` without heap allocation.
It has no dependency on the application, a board, an executor, or an operating system.
The crate does not contain the complete platform implementation.

The firmware and simulator contain backend implementations and runtime code.
A backend controls its drivers, buffers, resource access, and completion events.
The runtime connects an application to these services.
Only this connection requires both the application and backend code.

## Current application loop

The runtime serializes calls into `App`.
It supplies the clock, display, discovery, and location services.
Application state stays on one core.
The runtime controls executor selection, waits, event delivery, and driver supervision.

The runtime has these tasks:

- Process driver completions and input events.
- Call `App::poll_discovery` with the discovery service.
- Call `App::poll_location` with the location service.
- Call `App::tick` with the clock.
- If a display is available, call `App::present` through the coordinate adapter.
- Wait for the earliest application deadline, backend deadline, input event, or driver completion.

The simulator advances virtual time through deadlines in order.
Firmware uses its runtime clock and event sources.
The [simulator guide](../simulator.md) describes deterministic execution.
[Backend profiles](../hardware.md) describe hardware execution, core assignment, and resource ownership.
