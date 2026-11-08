# Shared bus ownership

The platform [bus arbitration helper](../../crates/platform/src/bus.rs) gives asynchronous and synchronous clients exclusive access to a shared value.
It contains no I2C driver, board pins, executor, timer, or device policy.
The backend supplies the value, storage, raw mutex, and queue capacity.

## Ownership and waiting

`Arbitrated::lock` queues asynchronous clients through a fair semaphore with one permit.
Only the next client receives a wake notification when the current owner releases access.
This prevents interrupt tasks from repeatedly waking while another client owns the bus.
The guard releases the value before it releases the permit and wakes the next client.

`N` bounds queued waiters. Exceeding that bound panics instead of silently losing a waiting client.
The backend must reserve enough slots for concurrent acquisition attempts.
Canceling an acquisition future releases its queue slot. Dropping a guard releases ownership.
Canceling acquisition does not cancel a device transaction that already holds a guard.

`try_lock` returns immediately and does not bypass queued clients.
`lock_blocking` retries through a caller-supplied callback that controls pacing and the retry budget.
That callback must permit the current owner to progress.
A synchronous client must not block an executor that needs to poll the owner before it can release access.

## Backend integration

The [firmware bus module](../../firmware/src/bus.rs) owns I2C configuration, board pins, static storage, and the client capacity.
It selects critical-section locking for coordination across interrupt tasks and processor cores.
The mutex masks interrupts only while it changes lock state. A guard keeps the bus exclusive with interrupts enabled.
The [display and touch integration](../boards/lilygo-t5-s3-pro-touch.md#concurrent-display-and-touch) uses this boundary for asynchronous input and synchronous display controls.

Bus ownership and [worker pause coordination](worker-coordination.md) have separate purposes.
A worker must finish its transaction and release its guard before acknowledging a lifecycle pause.
The bus helper does not initiate power transitions or decide when a device is safe to stop.

## Embassy primitives

The helper combines Embassy's async `Mutex` with its `FairSemaphore`. It does not implement a separate wait queue.
The semaphore queues multiple clients. The mutex provides safe access to the shared value after permit acquisition.
The [pinned Embassy mutex](https://docs.rs/embassy-sync/0.8.0/src/embassy_sync/mutex.rs.html) stores one waiting-task waker.
Competing waiters can replace that waker and repeatedly wake each other before ownership becomes available.
The semaphore prevents that contention by admitting only one client to the value mutex.

Embassy also supplies [shared I2C adapters](https://docs.embassy.dev/embassy-embedded-hal/0.6.0/default/shared_bus/index.html).
Its async adapter requires an async I2C driver. Firmware currently uses blocking transactions and synchronous display callers.
Its blocking adapter holds a blocking mutex throughout each transaction.
With firmware's critical-section mutex, that would mask interrupts during the transfer.
The current combination permits asynchronous waits and bounded synchronous retries while keeping interrupts enabled during transactions.

## Verification limits

Native tests cover notification counts, queue order, cancellation, released queue slots, and bounded synchronous retries.
Firmware checks verify the hardware type aliases and their callers.
These tests do not establish hardware timing or interrupt latency.
