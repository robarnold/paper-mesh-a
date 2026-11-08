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

## Firmware execution placement

The firmware runtime selects cores, executors, software interrupts, and worker priorities.
The [execution module](../../firmware/src/execution.rs) owns explicit executor storage and the display stack.
Its `Execution` owner receives CPU control and the three executor-related software interrupts together at initialization.
Start methods consume the correct resources internally. The display core and its scheduler interrupt remain paired.
Each executor can start only once. Construction alone does not start a core or enable an interrupt.
The [entry point](../../firmware/src/main.rs) starts the RTOS scheduler and supplies the main executor's spawner to ordinary service workers.
Adapters accept the selected spawner or expose a worker future. They do not allocate another executor.

| Work | Execution context | Reason |
| --- | --- | --- |
| Application, USB, RTC, power observations, buttons, discovery | Main executor on core 0 | Runtime serializes application access and ordinary service work |
| Touch reports | Interrupt executor on core 0, priority 2 | Input must progress during synchronous application rendering |
| GNSS input | Interrupt executor on core 0, priority 1 | UART consumption must progress during synchronous application rendering |
| Display refresh | Thread-mode executor on core 1 | Synchronous panel scans must not stop core 0 input processing |

The runtime starts touch and GNSS executors on core 0 and supplies their `SendSpawner` handles to the adapters.
A spawner identifies an executor. Sending the handle does not move that executor to the caller's core.
If touch initialization fails, its executor remains empty. The runtime omits the unavailable touch lifecycle participant.
Display task creation runs inside the core 1 executor's initialization callback.
Core parking still depends on the display worker reaching its existing lock-free checkpoint.

The RTOS uses software interrupts 0 and 1. The touch and GNSS executors use 2 and 3, respectively.
These assignments consume the board's four software-interrupt resources. Another interrupt executor requires an explicit resource decision.
Native tests cannot establish interrupt latency, stack headroom, or physical sleep reliability.

### Interrupts and async execution

Embassy futures do not each require an OS thread or a private thread stack.
This firmware uses [esp-rtos](https://docs.espressif.com/projects/rust/esp-rtos/0.4.0/esp32s3/esp_rtos/index.html), which supplies scheduling and radio runtime support.
Its thread-mode Embassy executors run on RTOS threads. An interrupt executor polls futures from a software-interrupt handler.

An enabled hardware interrupt can preempt current execution, subject to interrupt masking and priority.
Interrupt entry code saves context, invokes a handler, and restores execution on return.
A peripheral handler can acknowledge an event and wake a future. The selected executor determines where that future next runs.
The peripheral interrupt priority and the interrupt executor priority are separate settings.

The current touch worker performs synchronous I2C reads in interrupt context. The GNSS worker parses receiver input there.
A long poll can delay lower-priority interrupts and ordinary thread execution. Worst-case poll duration remains unverified.
Moving this work to scheduled threads requires a latency review because the main cooperative executor also performs synchronous application rendering.

Peripheral routing remains a separate responsibility from executor placement.
HAL driver initialization can bind interrupts on the core where initialization occurs.
The touch module still installs the shared GPIO handler and dispatches suspend wake events.
Central executor ownership does not change that handler, pin routing, or peripheral interrupt configuration.

## Current application loop

The runtime serializes calls into `App`.
It supplies the clock, display, discovery, location, and power services.
Application state stays on one core.
The runtime controls executor selection, waits, event delivery, and driver supervision.

Firmware initializes application storage with `App::unstarted` in static RAM.
It calls `App::start` once to build retained components in place and enter Home.
This avoids a complete application temporary on the task stack.
Native callers can use `App::new_at`, which performs both steps.

The UI system uses a bounded [layer store](composition.md) with separate committed and staging buffers.
It reclaims inactive page layers and hidden keyboard layers to fit internal RAM.
Pages retain their models and elements across these changes. Returning to a page publishes fresh layer handles.
The native guide stores its independent sessions on the host heap. Shared UI state does not require a heap.

The runtime has these tasks:

- Process driver completions and input events.
- Call `App::poll_discovery` with the discovery service.
- Call `App::poll_location` with the location service.
- Call `App::poll_power` with the power service.
- Call `App::tick` with the clock.
- If a display is available, call `App::present` through the coordinate adapter.
- Wait for the earliest application deadline, backend deadline, input event, or driver completion.

The simulator advances virtual time through deadlines in order.
Firmware uses its runtime clock and event sources.
The [simulator guide](../simulator.md) describes deterministic execution.
[Backend profiles](../hardware.md) describe hardware execution, core assignment, and resource ownership.
