# ESP GPIO readiness provider

The reusable [GPIO provider](../../crates/esp-adapters/src/readiness/gpio.rs) accepts a runtime-configured pin and active level.
Firmware supplies an input-enabled pin with its output driver disabled and gives the provider stable storage.
The provider splits into a readiness signal, a separate wake handle, and an interrupt registration.
Firmware registers and enables interrupt delivery before calling `start_capture`. It separately selects the worker executor.
The provider does not know the connected device or choose an executor.

The provider owns interrupt masking, acknowledgement, and rearming.
Capture continues without a waiting reader, including while acquisition admission is blocked.
`did_interrupt` masks and acknowledges the pin, then retains one notification. Repeated observations may coalesce with that notification.
Masking a retained level prevents repeated interrupt entry while the consumer cannot proceed.
Consuming the notification rearms capture. An active level produces another notification without requiring a new edge.
Thus, a readiness notification can cause a device check that finds no new report.

`wait_for_readiness` has exclusive use of ordinary readiness delivery.
The platform `Readiness::wait` implementation delegates to this provider method.
`did_end_delivery` releases reader ownership after completion or cancellation.
Canceling the wait preserves capture and pending readiness.
Short pulses entirely within masked periods are not guaranteed to survive. This provider is intended for a level held until device service.
Hardware interrupt delivery and execution of the acquisition worker in an interrupt executor remain independent decisions.

## Wake control

Wake control is separate from the platform [readiness contract](../platform/readiness.md).
The coordinator must quiesce acquisition before arming wake capture.
`arm_wake_capture` returns `Result<(), ArmError>`. It rejects an active reader, active input, or a provider outside ordinary capture mode.
Refusal leaves provider state unchanged. Earlier readiness remains pending when a new wake interval starts.

`finish_wake_capture` returns `Option<WakeCapture>`. `None` means that no interval was armed, including repeated completion.
An armed interval returns `Captured` or `NotCaptured`. Neither observation is an operation failure.

A wake callback records capture before notifying the system coordinator. It then leaves the pin masked.
Wake completion also inspects a pending hardware latch before clearing it, covering interrupts whose callback has not run.
Captured state survives completion and ordinary readiness delivery. Repeated completion does not report the same wake again.
The consumer takes the restoration observation before releasing acquisition admission.
This observation includes retained wake capture and the current input level.

`take_restoration_observation` returns retained wake evidence and the current input level as separate fields.
It consumes wake evidence without consuming ordinary readiness. Repeated calls still sample the current level.

The coordinator calls `finish_wake_capture` before service restoration to record wake reasons.
The consumer later calls `take_restoration_observation` before releasing acquisition admission.
These operations remain separate because they have different callers and occur at different stages.
The current input level is sampled during restoration, rather than fixed when the wake interval ends.
The wake handle does not configure system sleep or choose wake policy.

## Verification and limits

Provider tests use fake pin latches to cover active input, rearming, coalescing, independent providers, cancellation, and retained wake capture.
Firmware checks compile the concrete GPIO provider and runtime connections.
These checks do not establish physical wake delivery, interrupt latency, or timing margins on the board.
