# PCF8563 calendar adapter

[Time contract](../services/time.md) · [Hardware backends](../hardware.md)

## Scope

The [PCF8563 driver](../../crates/pcf8563/src/lib.rs) accesses calendar registers through `embedded-hal` I2C.
It implements the platform `Rtc` contract and contains no ESP peripherals or executor dependencies.
The caller supplies the I2C handle and schedules device access.
The driver can retain an owned handle or borrow a bus for a single operation group.
The platform [synchronization service](../services/time.md#calendar-synchronization) uses the contract without a dependency on this driver.
Firmware connects the service to this device and supplies its policy.

## Calendar access

The driver reads control state and the complete calendar in one transaction.
STOP, test mode, voltage loss, malformed BCD, or invalid calendar fields invalidate UTC.
The adapter assigns century bit `C=0` to years 2000 through 2099.
It rejects `C=1` and dates outside that range.
Retained values require this century convention.

The driver sets STOP, writes the complete calendar, and clears STOP.
It leaves alarm, timer, and clock-output registers unchanged.
A failed calendar write leaves STOP set to prevent use of a partial date.
The synchronization service reads the calendar after a correction.
This read checks calendar validity but does not compare the result with the requested time.

## Evidence and limits

The [NXP PCF8563 datasheet][rtc], revision 11.1, defines control registers in section 8.3 and calendar validity in section 8.4.
Section 8.5 requires one complete calendar transfer.
Section 8.10 describes STOP and the first increment after restart.

[Native tests](../../crates/pcf8563/tests/rtc.rs) cover register encoding, validity flags, partial writes, corrections, retry, and retained startup without location observations.
These tests do not establish physical retention, oscillator accuracy, or synchronization accuracy.
RTC interrupts, alarms, and sleep integration remain planned work.
Board profiles record physical integration and validation status.

[rtc]: https://www.nxp.com/docs/en/data-sheet/PCF8563.pdf
