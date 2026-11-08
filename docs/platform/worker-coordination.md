# Worker coordination

The platform [pause channel](../../crates/platform/src/worker.rs) coordinates one controller and one worker without a service registry.
It contains shared state and notifications. It does not own a task, timer, bus, or power transition.
The runtime creates its storage and supplies each endpoint to its owner.

## Roles and ownership

| Role | Responsibility |
| --- | --- |
| Worker | Async operation that performs service work and reaches safe checkpoints |
| Service control handle | Runtime-owned object that exposes service operations and lifecycle capabilities |
| Lifecycle participant | Service control handle that implements a suspend or power-off contract |
| Pause controller | `PauseControl`, the endpoint that requests, awaits, and releases a worker pause |
| Pause worker | `PauseWorker`, the endpoint that observes requests and acknowledges safe checkpoints |
| System coordinator | Backend code that orders lifecycle operations, enforces deadlines, and performs the power transition |

A lifecycle participant can use a pause channel to stop its worker before it changes service state.
A pause acknowledgement alone does not establish hardware shutdown or fresh observations.
The participant owns those service-specific requirements.
The [power contracts](../services/power.md#suspend-participants) define suspend and power-off behavior separately.

`Pause::split` requires an exclusive borrow and returns one endpoint of each kind.
The endpoints cannot be cloned. Their mutable methods prevent simultaneous waits on the same endpoint.
Storage must outlive both endpoints. Dropping a handle does not release an active request.
The runtime retains the handles for the service lifetime.

## Pause protocol

1. The controller requests a pause. A request does not acknowledge resource release.
2. The worker finishes its current operation, releases resources, and publishes completed work.
3. At `checkpoint`, the worker acknowledges the pause and waits.
4. The controller's `pause` future completes only after this acknowledgement.
5. `resume` releases the worker. A later request needs a new acknowledgement.

Release permits work but does not establish that the task has run again.
It does not refresh observations or perform device initialization.

`request_pause` submits the request without waiting, for paths such as reset recovery.
`PauseWorker::requested` can interrupt an idle wait when raced against a timer, queue receive, or other event.
The normal operation must permit cancellation when the caller races it against a pause request.
Workers need no timer loop or separate OS task. Their futures and the controller must receive concurrent polling.
An executor can run them as separate tasks, or one task can poll them together.
Waiting yields the future. It does not block the executor thread.
The RTC uses this to reach its checkpoint without waiting for another calendar poll.
The mechanism does not interrupt an operation already in progress. The system coordinator bounds preparation with its lifecycle deadline.

Notifications wake the endpoints, but shared state determines whether a request or acknowledgement is current.
Repeated requests are idempotent while a pause remains active.
Coalesced signals cannot lose a request. Stale signals cannot complete a later pause.

Canceling the controller's wait leaves the pause requested.
Suspend cancellation must explicitly release that request, including when the worker has not yet acknowledged it.
Power-off failure keeps the request active until reset.
Canceling a worker checkpoint revokes its acknowledgement. The worker must reach another checkpoint before it continues work.
Callers must not cancel a checkpoint and bypass a pending pause request.

## RTC integration

The [firmware runtime](../../firmware/src/main.rs) creates one channel with `CriticalSectionRawMutex` and splits it during startup.
It gives `PauseControl` to `FirmwareClock` and `PauseWorker` to the RTC task.
The runtime retains that clock handle for cached reads and lifecycle calls.
The [RTC module](../../firmware/src/rtc/mod.rs) has no dependency on the firmware suspend registry.

Before acknowledging a checkpoint, the worker releases the I2C guard and publishes its synchronization state.
The clock handle then invalidates cached UTC while the worker remains paused.
After suspend or canceled preparation, it invalidates the cache before releasing the pause.
The worker reads the RTC on its next iteration. Consumed correction history remains intact.
This separates worker availability from calendar validity.

Power-off preparation leaves the worker paused until power removal or reset.
Preflight fault paths explicitly request its pause before starting reset recovery.
This request does not claim that the worker has already stopped.

## USB integration

The [USB lifecycle handle](../../crates/esp-adapters/src/usb/lifecycle.rs) owns four pause controllers.
USB startup supplies their worker endpoints to the device, HID, log, and command tasks.
These workers no longer use the global suspend registry.
The reply transport remains a separate capability. Sending a reply does not require lifecycle ownership.

Preparation requests the three endpoint workers first and waits for all acknowledgements.
Their pause requests cancel pending endpoint futures before acknowledgement.
Command cancellation also discards an incomplete request frame.
The device worker then detaches the PHY and disables the USB stack before its checkpoint.
A pause request during the reconnect delay keeps the device detached.

Restoration invalidates the previous session and queued replies, then requests USB reconnection before releasing the workers.
This also applies after partial preparation, even if the device has not yet detached.
Endpoint workers wait for a new configured connection before they access the device.
Restoration permits reconnection. It does not promise that a host has connected.
Power-off preparation leaves all workers paused until power removal or reset.

## Runtime composition

The [runtime service handles](../../firmware/src/lifecycle.rs) combine the RTC and USB lifecycle capabilities.
The hardware suspend coordinator still accepts one generic participant and has no dependency on either adapter.
The runtime prepares USB before the RTC and restores the RTC before USB.
It passes the same coordinator deadline to both services.
Cancellation restores both handles because preparation can stop at any await.
Both current implementations are infallible. General error aggregation and dependency planning remain separate work.

Reset recovery requests both services to pause without waiting for acknowledgement.
This path does not claim safe device detachment. The watchdog returns through normal startup.
Buttons, GNSS, power, and touch still use the global `Service` registry.
The system coordinator waits for those workers in addition to the runtime participant.
Display-core parking is a hardware execution barrier and remains distinct from a cooperative worker pause.

## Implementation and verification

The channel uses `embassy-sync` without an executor or clock dependency.
The backend selects a `RawMutex` appropriate for its execution context.
Firmware uses critical-section locking. Native tests use a single-thread lock and directly poll futures.
No lock is held across an await, and the channel requires no heap allocation.

Tests cover both request orderings, canceled waits, revoked acknowledgements, rapid release and re-request, independent channels, and endpoint wakeups.
USB lifecycle tests cover endpoint ordering, partial cancellation, session invalidation, and reset pause requests.
Firmware checks verify runtime integration. These checks do not establish physical sleep or wake reliability.

The [platform manifest](../../crates/platform/Cargo.toml) pins the synchronization dependency.
The [reviewed Embassy source](https://github.com/embassy-rs/embassy/tree/ca966f8ff4ee81c14bccade56b7a52c1fc154e26) offers MIT or Apache-2.0 licensing.
The MIT option permits redistribution with its copyright and permission notice.
Distribute the [MIT notice](../../crates/platform/licenses/LICENSE-embassy-MIT) with products that include this dependency.
