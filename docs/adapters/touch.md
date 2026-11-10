# ESP touch adapter

The [touch adapter](../../crates/esp-adapters/src/touch.rs) connects GT911 reports and readiness notifications to contact frames and HOME observations.
It currently supports the fitted H752-01 controller on ESP32-S3. Other controllers and boards are not verified.
The [board profile](../boards/lilygo-t5-s3-pro-touch.md) defines wiring, controller configuration, and measured limits.

## Ownership

The GT911 driver owns register access and raw contact reports.
The ESP adapter converts those reports into the platform [contact-frame contract](../services/input.md#contact-frame-contract).
It owns contact continuity, HOME observations, reset, event queues, statistics, and acquisition coordination.
Its lifecycle participant prepares logical input after acquisition quiescence and restores it before admission resumes.
The separate GPIO readiness provider owns interrupt capture and pin acknowledgement, including while acquisition is blocked.
The runtime supplies pins for initialization, a readiness signal for acquisition, portrait geometry, HOME delivery, restoration observations, and display activity diagnostics.
The adapter has no dependency on firmware button, display, or suspend modules.
The [acquisition contract](../platform/readiness.md#acquisition-control) defines driver quiescence and admission control.

Controller initialization returns the device and its interrupt pin after reset and configuration checks.
Firmware transfers the pin to a GPIO readiness provider and connects its readiness signal to the touch worker.
Worker preparation returns workers and the lifecycle handle. Firmware registers the provider callback before launching the worker.
The provider's wake handle is connected separately to system suspend coordination.

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
Frame timestamps use the readiness observation on the runtime monotonic clock.
An already-active level uses its observation time without requiring a new interrupt. Existing interrupt-count diagnostics now count readiness observations.
HOME is delivered separately through the runtime callback, without an invented contact position.

A failed read invalidates HOME. The first consecutive failure clears queued frames and delivers cancellation.
Queue overflow also replaces pending input with cancellation. The GT911 driver suppresses held contacts until an empty report confirms release.
The adapter keeps the GT911-required 1 ms delay between report attempts. The platform contract imposes no polling interval.

## Readiness and suspend

The touch acquisition worker consumes this contract and reads the GT911 through its protocol driver.
The driver owns I2C reads, report decoding, and acknowledgement of consumed device reports.
It does not depend on GPIO or the readiness provider.
The platform contact-frame contract remains the output boundary for logical touch input.

The touch suspend participant requests quiescence, then clears queued reports and inspects held contacts and HOME state.
The worker returns its device to shared storage before its next wait. No critical section spans an I2C acquisition.
During restoration, the participant updates contact state before releasing admission.
The existing device retry delay remains in the worker and can delay acknowledgement.

The worker consumes the portable [readiness contract](../platform/readiness.md).
The [GPIO provider](gpio-readiness.md) owns pin masking, acknowledgement, rearming, and active-level detection.
GT911 initialization completes reset and address selection before the pin enters provider ownership.
The adapter retains its ESP I2C driver and current core 0 requirement.

The firmware [touch wake connection](../../firmware/src/features/touch_wake.rs) combines contact-idle admission with the separate GPIO wake handle.
The coordinator arms wake only after acquisition quiescence and touch preparation.
During restoration, the participant consumes retained wake capture and checks the current input level before releasing acquisition admission.
GT911 contact suppression requires release before accepting a new contact sequence after uncertain suspend input.

If preparation is canceled before quiescence, restoration releases the request without accessing device state.
The `prepared` flag distinguishes this case from completed logical preparation. First boot does not use the resume path.

## Verification

Native checks exercise the GT911 protocol, readiness cancellation and wake capture, platform coordinate conversion, and contact-state handling.
Firmware checks compile the extracted adapter, lifecycle integration, interrupt callback, and concrete task with stack-size checks enabled.
These checks do not establish physical interrupt delivery, touch latency, or sleep reliability.
No device is flashed by the checks.
