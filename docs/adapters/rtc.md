# PCF8563 calendar adapter

[Time contract](../services/time.md) · [Hardware backends](../hardware.md)

## Scope

The [PCF8563 driver](../../crates/pcf8563/src/lib.rs) accesses calendar registers through `embedded-hal` I2C.
It implements the platform `Rtc` contract and contains no ESP peripherals or executor dependencies.
The caller supplies the I2C handle and schedules device access.

The PCF8563 has a fixed seven-bit I2C address of `0x51`, independent of the board.
The [datasheet][rtc] gives `0xA2` for writes and `0xA3` for reads. These bytes include the read/write bit.
The driver keeps this address constant because the chip does not support address selection.

The driver can retain an owned handle or borrow a bus for a single operation group.
The platform [synchronization service](../services/time.md#calendar-synchronization) uses the contract without a dependency on this driver.
The ESP clock adapter connects the service to this device. Firmware supplies its policy.

## ESP clock integration

The [ESP clock adapter](../../crates/esp-adapters/src/rtc.rs) implements the platform clock, suspend, and power-off capabilities.
It combines the PCF8563 driver with the platform synchronization service and Embassy monotonic time.
The hardware feature selects ESP dependencies. The PCF8563 driver and synchronization algorithm remain separate from ESP hardware.

The runtime initializes `ClockState` with its policy, then splits matching `RtcClock` and `ClockWorker` endpoints.
It owns static storage and supplies a location callback. The adapter has no dependency on the GNSS implementation.
The runtime also owns bus initialization, pins, frequency, timeout, and queue capacity.

`ClockWorker::prepare` accepts the shared ESP I2C bus with its caller-selected queue capacity and returns `Workers`.
The adapter owns the Embassy task and its storage. `Workers` implements the platform `WorkerSet` contract.
Firmware launches it through `Execution::spawn`, which supplies the main executor's spawner.
The adapter owns the polling loop and waits for deadlines selected by the platform synchronization service.
This separates executor selection from service timing without another scheduling policy.

Preparation requires an initialized allocator and allocates the complete worker future once.
The boxed future retains the concrete bus reference and hides its capacity from the task type.
Polling uses dynamic dispatch but adds no allocation per calendar read or synchronization cycle.
The adapter owns this future for the task's lifetime. Shared-bus arbitration and lifecycle ordering remain unchanged.

The [pause protocol](../platform/worker-coordination.md#rtc-integration) requires completed access and cache publication before acknowledgement.
Suspend restoration invalidates cached UTC before worker release. Power-off preparation leaves the retained hardware counter running.
Clock reads never acquire the I2C bus. Before the first successful poll, calendar reads return no UTC.

## Calendar access

The driver reads control state and the complete calendar in one transaction.
STOP, test mode, voltage loss, malformed BCD, or invalid calendar fields invalidate UTC.
The adapter assigns century bit `C=0` to years 2000 through 2099.
It rejects `C=1` and dates outside that range.
Retained values require this century convention.

The driver functions and the `Rtc` implementation return the same `RtcError` operation errors.
A successful read returns usable UTC. Invalid retained time returns `InvalidTime` with a reason.
Overlapping conditions report voltage loss first, then test mode, STOP, unsupported century, and invalid calendar fields.
An I2C failure returns `Access` without an inference about calendar state.
Unsupported input years return `UnsupportedDate` before any bus access.

The driver sets STOP, writes the complete calendar, clears STOP, and reads the result.
It leaves alarm, timer, and clock-output registers unchanged.
A failed calendar write leaves STOP set to prevent use of a partial date.
Readback must contain valid calendar time equal to the request or one second later.
This tolerance permits one counter tick during immediate verification, including date rollover.
Older time, a larger difference, or invalid calendar state produces `VerificationFailed`.
A failed verification read preserves the `Access` error.
An error does not imply that the write left the device unchanged.
The synchronization service then reads a fresh sample for cached UTC.
Verification does not establish power-loss retention or oscillator accuracy.

## Evidence and limits

The [NXP PCF8563 datasheet][rtc], revision 11.1, defines control registers in section 8.3 and calendar validity in section 8.4.
Section 8.5 requires one complete calendar transfer.
Section 8.10 describes STOP and the first increment after restart.

[Native tests](../../crates/pcf8563/tests/rtc.rs) cover register encoding, validity flags, partial writes, corrections, retry, and retained startup without location observations.
These tests do not establish physical retention, oscillator accuracy, or synchronization accuracy.
RTC interrupts, alarms, and sleep integration remain planned work.
Board profiles record physical integration and validation status.

[rtc]: https://www.nxp.com/docs/en/data-sheet/PCF8563.pdf
