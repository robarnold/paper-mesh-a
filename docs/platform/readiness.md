# Device readiness

The platform [Readiness contract](../../crates/platform/src/readiness.rs) means that device data may be ready to read.
A notification requests a device check. It does not promise exactly one report or count queued device reports.
Notifications may coalesce. A condition already active when waiting begins must not be missed.
The observation carries microseconds on the runtime monotonic clock so consumers can retain acquisition timing.
Canceling a wait stops that delivery attempt. It must not discard captured readiness.

## Acquisition control

New acquisition channels block delivery until the owner calls `resume` after startup dependencies are ready.

The platform [acquisition channel](../../crates/platform/src/acquisition.rs) separates readiness capture from permission to acquire data.
It supplies one reader and one control handle. It uses the [pause channel](worker-coordination.md) internally.
Quiescence means that no acquisition is in progress and no new acquisition can start until release.
It does not mean that consumer state is reset, queues are empty, or hardware capture has stopped.

The reader must release acquisition resources and publish its result before polling its next `wait`.
Polling that wait acknowledges a pending quiescence request. Constructing an unpolled future does not acknowledge it.
A reader already waiting for readiness can acknowledge without acquiring data first.
The controller waits for this acknowledgement before accessing state owned by the acquisition loop.

A captured notification remains available while admission is blocked, including when readiness and a quiescence request occur together.
Dropping a reader wait revokes its acknowledgement. The consumer must poll a new wait before acquiring again.
Canceling the controller's wait leaves its request active. Only explicit resume releases admission.
These rules require cooperative callers. The channel cannot detect resources retained outside its reader.

Suspend motivates this separation: consumers can prepare logical state while capture retains notifications.
Reset sequencing, wake-source admission, and hardware wake capture belong to the backend and runtime.

## Implementations and verification

The [ESP GPIO provider](../adapters/gpio-readiness.md) implements readiness capture and separate wake control.
The [touch adapter](../adapters/touch.md#readiness-and-suspend) consumes readiness through acquisition control.
Native acquisition tests cover unpolled waits, requests during acquisition, idle readers, cancellation, retained readiness, and repeated requests.
