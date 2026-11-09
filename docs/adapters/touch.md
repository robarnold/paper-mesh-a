# ESP touch adapter

The [touch adapter](../../crates/esp-adapters/src/touch.rs) connects the GT911 protocol to contact frames, HOME observations, and GPIO wake handling.
It currently supports the fitted H752-01 controller on ESP32-S3. Other controllers and boards are not verified.
The [board profile](../boards/lilygo-t5-s3-pro-touch.md) defines wiring, controller configuration, and measured limits.

## Ownership

The GT911 driver owns register access and raw contact reports.
The ESP adapter converts those reports into the platform [contact-frame contract](../services/input.md#contact-frame-contract).
It owns contact continuity, HOME observations, reset, event queues, statistics, GPIO acknowledgement, and worker pause coordination.
The runtime supplies pins, portrait geometry, HOME delivery, wake callbacks, and display activity diagnostics.
The adapter has no dependency on firmware button, display, or suspend modules.
The [worker coordination contract](../platform/worker-coordination.md#touch-input-integration) defines pause and restoration behavior.

Preparation returns the GPIO registration, worker resources, and lifecycle handle together after successful controller initialization.
Firmware registers the callback and enables delivery before launching the worker.
The callback acknowledges only its own pin. The worker rearms that pin after processing.
Wake capture runs in interrupt context through the supplied callback, which must not wait or access devices.

Preparation accepts the shared ESP I2C bus with a runtime-selected queue capacity and returns a capacity-independent `Workers` value.
The adapter owns the concrete Embassy task and its storage. Firmware calls `workers.spawn(selected_spawner)` without naming the task.
Firmware selects the executor and retains core 0 placement. The adapter currently requires preparation and worker execution on core 0.

Preparation makes one heap allocation for the complete worker future, after the allocator is initialized.
That future retains the concrete bus reference and its capacity internally. Polling uses dynamic dispatch but makes no additional allocations.
The bus, queue, and guard remain unchanged, including FIFO ordering, cancellation cleanup, and release before waking another client.
This avoids an allocated future for each bus acquisition or custom unsafe storage for an erased guard.
The allocation remains owned by the worker for its lifetime.

USB uses the platform `WorkerSet` contract with a local spawner. Touch accepts `SendSpawner` directly because it runs on an interrupt executor.
RTC still exposes a worker future for its runtime-owned concrete task.

## Contact delivery

Each queued sample carries a platform `TouchFrame` plus adapter diagnostics. Consumers do not receive GT911 report types.
The configured transform maps controller coordinates into the logical portrait space used by the application and touchpad consumer.
Frame timestamps use the captured GPIO event time on the runtime monotonic clock.
HOME is delivered separately through the runtime callback, without an invented contact position.

A failed read invalidates HOME. The first consecutive failure clears queued frames and delivers cancellation.
Queue overflow also replaces pending input with cancellation. The GT911 driver suppresses held contacts until an empty report confirms release.
The adapter keeps the GT911-required 1 ms delay between report attempts. The platform contract imposes no polling interval.

## GPIO boundary

The current GPIO implementation uses esp-hal pins for reset, address selection, interrupt masking, acknowledgement, and wake arming.
Standard `embedded-hal` traits cover input reads and output levels. They do not specify direction changes or interrupt acknowledgement.
The GT911 reset sequence temporarily drives the interrupt pin to select the device address.
A portable GPIO implementation would therefore need additional direction and interrupt capabilities.
That boundary remains ESP-specific in this extraction. Replacing it with an async GPIO wait would require a separate pause and wake review.

## Verification

Native checks exercise the GT911 protocol, platform coordinate conversion, and contact-state handling.
Firmware checks compile the extracted adapter, lifecycle integration, interrupt callback, and concrete task with stack-size checks enabled.
These checks do not establish physical interrupt delivery, touch latency, or sleep reliability.
No device is flashed by the checks.
