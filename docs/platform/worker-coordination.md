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

New and default channels start with admission blocked. This initial request does not acknowledge worker quiescence.
The owner calls `resume` when startup dependencies are ready, without invoking suspend restoration.
RTC, USB, touch, button polling, and power observation release admission during setup before worker launch.
GNSS waits for the auxiliary coordinator to establish power before release.
The rail admission channel and GNSS lifecycle channel remain separate: rail admission does not prove UART sleep preparation.

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

The [firmware runtime](../../firmware/src/main.rs) supplies synchronization policy when it initializes `ClockState` in static storage.
This state contains the calendar cache and one pause channel with `CriticalSectionRawMutex`.
Its exclusive `split` returns a clock handle and a worker endpoint that share the same cache and policy.
It gives `PauseControl` to `RtcClock` and `PauseWorker` to `ClockWorker`, which the runtime passes to the RTC task.
The runtime retains that clock handle for cached reads and lifecycle calls.
The [RTC adapter](../../crates/esp-adapters/src/rtc.rs) receives pause requests only through this endpoint.
The firmware task runs the adapter loop with its concrete bus type. The adapter accepts the bus queue capacity as a const parameter.
If bus initialization fails, the clock still supplies monotonic time. Its unpopulated calendar cache returns no UTC.

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
USB preparation supplies their worker endpoints to the device, HID, log, and command workers.
The runtime launches every prepared worker before it uses the lifecycle handle.
Each task receives its own pause endpoint.
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

## Power observation integration

The [power observation module](../../firmware/src/power/observation.rs) starts its periodic worker and returns a `PowerObservationLifecycle` handle.
The runtime supplies a nonzero polling interval. The worker waits that interval after each completed pair of device reads.
Startup creates this handle only when shared-bus initialization succeeds.
The runtime omits an unavailable worker from lifecycle composition, so preparation cannot wait for a task that never started.

Preparation waits for battery and charger reads to finish, bus guards to drop, and the resulting snapshot to publish.
The pause request interrupts the polling delay, but does not cancel a device transaction.
Restoration releases the worker to take another sample. Cached observations retain their original timestamps until replaced.
The read-only `FirmwarePowerObservation` capability remains separate from lifecycle ownership.
Cached reads, change notifications, fault publication, and immediate reads belong to the observation module.
Power-off admission and battery isolation remain separate operations in the power control module.

Pausing observations does not switch off the charger, battery gauge, or board power.
The system coordinator retains direct `read_now` access for suspend comparisons while the periodic worker is paused.
Power-off preparation keeps the worker paused. The coordinator owns the hardware cut and reset recovery.

## Button polling integration

The [button module](../../firmware/src/buttons.rs) starts its polling worker and returns a `ButtonPollingLifecycle` handle.
The worker polls BOOT directly and Function through the shared bus when available.
Startup retains the handle even when bus initialization fails because BOOT polling still operates.

A checkpoint follows GPIO setup, completed reads, event publication, and bus release.
The pause request interrupts the polling delay. It does not cancel a bus transaction.
Restoration resumes polling with the existing debounce state and queued events.
Power-off preparation keeps polling paused until power removal or reset.

This handle does not stop HOME updates from the touch worker or arm wake sources.
The system coordinator waits for all required workers before it arms BOOT and expander wake interrupts.
Hardware wake capture remains active while polling is paused.
The runtime must coordinate touch separately when it needs all input workers stopped.

## Touch input integration

The touch adapter uses [acquisition control](readiness.md#acquisition-control) to await driver quiescence before logical input preparation.
Its lifecycle participant restores logical state before releasing acquisition admission.
The [touch adapter documentation](../adapters/touch.md#readiness-and-suspend) defines contact cleanup, canceled preparation, and wake integration.

## GNSS integration

The [runtime GNSS worker](../../firmware/src/gnss_worker.rs) returns a lifecycle handle when the shared bus is available.
Its worker runs on an interrupt executor. Its pause endpoint does not depend on a system service registry.
The read-only location capability remains separate from lifecycle ownership.

Preparation cancels UART reception and releases its RX wake lock.
The serial owner retains an idle transmitter because HAL sleep preparation requires UART1 clocks.
It publishes an unavailable receiver observation and its UART preparation result before acknowledging the pause.
The firmware rail coordinator then disables the shared auxiliary supply.
The lifecycle handle reads those results only after acknowledgement, so an earlier result cannot approve a new preparation.

UART setup or supply shutdown failure prevents the power transition.
Suspend cancellation releases the worker to restart the receiver. Power-off failure keeps it paused for reset recovery.
Restoration permits receiver initialization but does not establish a fresh location or UTC observation.
Firmware resets after failed initial rail enable while keeping GNSS admission blocked. UART retry remains in the GNSS worker.

## Runtime composition

The [runtime service handles](../../firmware/src/lifecycle.rs) combine RTC, USB, button polling, and available touch-input, power-observation, and GNSS lifecycle capabilities.
The hardware suspend coordinator still accepts one generic participant and has no dependency on the selected lifecycle handles.
The runtime prepares USB, touch input, button polling, power observations, RTC, then GNSS. Restoration uses the reverse order.
GNSS prepares last so its hardware failure leaves the other workers paused and cached UTC invalidated.
It passes the same coordinator deadline to each service.
Cancellation restores all present handles because preparation can stop at any await.
GNSS UART preparation and auxiliary-rail operations can fail. The other current participants are infallible.
The aggregate distinguishes consumer preparation failures from rail access failures. General error aggregation and dependency planning remain separate work.

Reset recovery requests all present services to pause without waiting for acknowledgement.
This path does not claim safe device detachment. The watchdog returns through normal startup.
All cooperative worker pauses use explicit handles. There is no global worker registry.
The coordinator retains a system transition flag to exclude discovery and new control operations.
That admission flag does not notify workers or acknowledge preparation.
Display-core parking is a hardware execution barrier and remains distinct from a cooperative worker pause.

## Implementation and verification

The channel uses `embassy-sync` without an executor or clock dependency.
The backend selects a `RawMutex` appropriate for its execution context.
Firmware uses critical-section locking. Native tests use a single-thread lock and directly poll futures.
No lock is held across an await, and the channel requires no heap allocation.

Tests cover both request orderings, canceled waits, revoked acknowledgements, rapid release and re-request, independent channels, and endpoint wakeups.
USB lifecycle tests cover endpoint ordering, partial cancellation, session invalidation, and reset pause requests.
GNSS lifecycle tests cover UART failure, acknowledgement ordering, retries, incomplete cancellation, and retained pauses after power-off failure.
Firmware checks verify runtime integration. These checks do not establish physical sleep or wake reliability.

The [platform manifest](../../crates/platform/Cargo.toml) pins the synchronization and executor dependencies.
The [reviewed Embassy source](https://github.com/embassy-rs/embassy/tree/ca966f8ff4ee81c14bccade56b7a52c1fc154e26) offers MIT or Apache-2.0 licensing.
The MIT option permits redistribution with its copyright and permission notice.
Distribute the [MIT notice](../../crates/platform/licenses/LICENSE-embassy-MIT) with products that include this dependency.
