# T5 S3 Pro real-time clock

[Board profile](lilygo-t5-s3-pro.md) · [Time contract](../services/time.md)

## Hardware adapter

The [calendar service](../../firmware/src/rtc/service.rs) advances UTC between shared-bus reads.
It limits extrapolation so an old sample cannot keep the clock apparently valid after communication stops.

The driver reads control state and the complete calendar in one transaction.
STOP, test mode, voltage loss, malformed BCD, or invalid calendar fields invalidate UTC.
The adapter assigns century bit `C=0` to years 2000 through 2099.
It rejects `C=1` and dates outside that range.
Retained values require this century convention.

The driver preserves valid RTC time at startup.
It does not initialize the calendar from a build timestamp.
The RTC continues without GPS reception after a valid correction.
Physical retention requires a suitable backup source on `VRTC`.

## GPS synchronization

A fresh RMC sentence must supply both date and time before it can correct the RTC.
The adapter accounts for elapsed time since receipt and consumes each successful correction once.
The correction threshold avoids repeated writes for small differences.

The driver sets STOP, writes the complete calendar, clears STOP, and reads the result.
It leaves alarm, timer, and clock-output registers unchanged.
A failed calendar write leaves STOP set to prevent use of a partial date.
Any I2C failure immediately invalidates the cached UTC value.
The next poll retries access, and a complete fresh report can restore a stopped or invalid calendar.

The backend interprets the RMC year as 2000 through 2099.
It does not infer a century from the build date.
It ignores fractional RMC seconds and does not use PPS.
The correction threshold limits repeated writes but does not establish accuracy.

## Evidence and limits

The [NXP PCF8563 datasheet][rtc], revision 11.1, defines control registers in section 8.3 and calendar validity in section 8.4.
Section 8.5 requires one complete calendar transfer.
Section 8.10 describes STOP and the first increment after restart.
The [GNSS adapter](lilygo-t5-s3-pro-gnss.md#motion-and-utc) identifies the RMC source and its validity rules.

Native tests cover register encoding, validity flags, partial writes, corrections, retry, and retained startup without GPS.
Physical I2C access, backup retention, oscillator accuracy, and GPS synchronization remain unverified.
RTC interrupts, alarms, and sleep integration remain planned work.

[rtc]: https://www.nxp.com/docs/en/data-sheet/PCF8563.pdf
