# LILYGO power observations

[Backend profile](lilygo-t5-s3-pro.md) · [Power contract](../services/power.md)

## Hardware and execution

The adapter reads the BQ27220 gauge at I2C `0x55` and the BQ25896 charger at `0x6B`.
Both devices use the shared bus on SDA39 and SCL40.
The [schematic][schematic], sheet 1, shows both devices and the 10 mΩ sense resistor.
The [pin map][pinmap] identifies bus pins and addresses.
The schematic shows no confirmed MCU route for charger `INT` or gauge `GPOUT`.

The worker runs on core 0. It waits two seconds after each pair of reads.
Each device read holds the bus mutex. The worker releases the mutex between devices.
The shared driver limits each I2C transaction to 20 ms.
Other clients can therefore delay observations. The interval is not a latency guarantee.
Each timestamp records the start of its device read.
The worker publishes completed observations and notifies the application loop.
A device failure removes only that device's data. The next poll attempts both devices again.

## Fuel gauge

The adapter uses little-endian standard commands from the [BQ27220 reference manual][gauge], table 2-1 and sections 2.6–2.28.
Current and average power use signed 16-bit values. Negative values indicate discharge.
The adapter converts internal temperature from 0.1 K to hundredths of a degree Celsius.
It reads `InternalTemperature()` at `0x28`, not the configurable `Temperature()` source at `0x06`.
It rejects percentage values above 100 and time estimates equal to `65535`.

`BATTPRES` supplies reported battery presence.
The fitted cell and gauge configuration determine the accuracy of this indication.
`INITCOMP`, `CFGUPDATE`, and `CALMD` determine measurement readiness.
The adapter omits measurements during startup, configuration, calibration, or reported battery absence.
It checks presence and readiness again after the measurements. A change rejects the complete sample.

The adapter reads existing capacity and health estimates without changes to the cell profile.
It does not unseal, calibrate, reset, or configure the gauge.
The backend marks estimates as unverified because software has not established the fitted cell parameters or calibration.
Gauge readiness alone cannot verify those conditions.

## Charger

The adapter uses the [BQ25896 datasheet][charger], sections 9.3.1 and 9.4.
`REG0B` supplies the input class, charge phase, power-good state, and minimum system voltage regulation.
`REG00`, `REG03`–`REG06`, and `REG0D` supply existing settings.
`REG0E` and `REG13` supply thermal and input regulation flags.
Input current can have a lower electrical limit than the register setting.

`VBUS_STAT` classifies input through `PSEL`, as section 9.2.3.3.1 specifies.
The [schematic][schematic], sheet 1, connects `PSEL` to ground, which selects the adapter class.
This class does not distinguish USB power from wireless power.
The schematic shows no separate signal that identifies wireless input.
The backend therefore cannot identify the physical power connection from these registers.

The adapter reads `REG0C` twice with separate transactions, as sections 9.2.9.3 and 9.3 specify.
The first read supplies latched faults. The second supplies current faults.
The thermistor field always reports its current state, including in the first read.
A read clears latched fault history. The application does not provide a persistent fault log.
The watchdog flag can indicate default mode after reset or watchdog expiry.

All writes contain only a register pointer for a read transaction.
The adapter does not change charge limits, watchdog settings, ADC mode, or BATFET controls.
Charger ADC measurements remain unavailable because this adapter does not start or verify a conversion.
Battery voltage and current come from the gauge.
Charge control and power removal remain planned work.

## Manual light sleep

The Suspend page requests explicit light sleep through `esp-hal 1.2.2`.
The RTOS uses its ordinary idle hook. It does not enter automatic light sleep.
A wake lock records application activity until suspend preparation completes.
The runtime permits ten seconds for the final display update and five seconds for service preparation.
An active or pending radio scan rejects the request.

Preparation pauses button, power, and RTC polling.
It stops touch interrupts, cancels the current touch gesture, and pauses USB reports and logs.
USB detaches before its device task acknowledges preparation.
The GNSS task drops the UART and its wake lock, then disables the shared GNSS/LoRa auxiliary supply.
It retains a separate idle UART1 transmitter with no attached pins during the pause.
This transmitter keeps UART1 clocks available without the receive wake lock.
HAL sleep preparation polls UART transmitters and can stop indefinitely if UART1 clocks are disabled.
The task drops the idle transmitter before it restores normal GNSS input.
The display has already issued its rail shutdown sequence before this step.
It also drops its temporary LCD and RMT drivers before it reports completion.
Both drivers retain clock wake locks for their lifetime. Rail shutdown alone does not release those locks.
The touch controller remains supplied. This experiment does not minimize every peripheral load.

The coordinator sets unconnected PCA9535 pins `IO0_1` through `IO0_7` as low outputs before it arms wake inputs.
It preserves the auxiliary supply on `IO0_0` and every port 1 signal.
These unused pins have no external connection on schematic sheet 3.
PCA9535 has no internal input pull resistors, so input mode can produce unwanted interrupts.
See [the board schematic](../vendor/lilygo/t5-s3-pro/README.md) and [PCA9535 section 4](https://www.ti.com/lit/ds/symlink/pca9535.pdf#page=3).
After services pause, the coordinator waits 500 ms for the display supply to settle.
TPS65185 discharge continues after PWRGOOD falls and WAKEUP goes low.
Its later status transition can assert the shared expander interrupt.
The delay precedes the battery sample and wake-input baseline.
See [TPS65185 Figure 2](https://www.ti.com/lit/ds/symlink/tps65185.pdf#page=13) and the vendor's
[shutdown delay before interrupt clearing](https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/lib/epdiy/src/board/epd_board_v7.c).
The coordinator then clears both input comparisons and arms low-level interrupts on GPIO0 and GPIO38.
BOOT uses GPIO0. Function uses PCA9535 `IO1_2`.
A held BOOT or Function control, or an asserted expander interrupt, rejects preparation.
GPIO38 is a shared expander interrupt, so another expander input can also end suspend.
If touch wake is enabled, the coordinator also arms GPIO3. A held contact, Home key, or asserted touch interrupt rejects preparation.
Touch wake uses the current GT911 configuration. Firmware does not change sensitivity or enable its gesture mode.
Contact sensitivity and unintended wake events require physical tests.
QON cannot wake this path through a software event.
The selected timer provides an independent wake deadline.

The interrupt handler records each observed wake input before it disables that interrupt.
After wake, the coordinator also captures pending interrupts before it reads the bus or resumes services.
It attributes GPIO38 to Function only if the Function input is still low.
A released control, another expander input, or a failed read produces `Control IRQ`.
GPIO3 produces `Touch/Home IRQ`. The report can include multiple inputs and an expired timer without assigning an event order.
The log includes the GPIO mask and both expander ports before and after sleep, when available.
It also records whether the coordinator called HAL sleep or skipped it because an input had already arrived.
The result page shows elapsed time to the nearest millisecond.
The touch driver consumes contacts from suspend until an empty report confirms release.

After services pause, the coordinator reads the gauge and charger directly before it arms wake inputs.
It releases the application wake lock only after every participating task acknowledges the pause.
Any remaining driver wake lock rejects the experiment.
Core 1 acknowledges a stop request between display operations, with its local interrupts disabled and no shared lock held.
The coordinator permits one second for this acknowledgement. Cancellation releases the request, including a concurrent acknowledgement.
Core 0 then disables its local interrupts and stalls core 1 at that known point.
It arms the RTC timer with the selected deadline and calls the HAL sleep function directly.
The HAL restores monotonic time after sleep. The coordinator captures pending GPIO events and releases the hardware stall.
It rearms the RTOS timer alarm before it enables interrupts and releases the cooperative stop.
After wake, the coordinator restores the wake lock and reads the gauge and charger before it resumes services.
USB reconnects. GNSS restarts baud selection and acquires new observations.
The display compares its framebuffer checksum across suspend and requires a full refresh after resume.

The [portable report](../services/power.md#manual-suspend-experiment) defines charge signs, retention, and measurement limits.
No external RTC register stores the report. No gauge or charger configuration changes occur.
An internal RTC memory word records suspend progress for diagnosis after a main-system reset.
A hardware reset can discard this marker.
USB logs report the previous and current stages when a terminal opens CDC with DTR asserted.
The log task waits for USB configuration and DTR before it accesses endpoints or removes queued messages.
The underlying driver's disable operation leaves old endpoint state until the next USB reset.
Each connection receives the latest wake diagnostics even if ordinary queued logs have been lost.
Retained progress identifies the last completed marker. It does not prove sleep residency or identify a fault inside a HAL call.

The RTC watchdog limits a failed experiment.
Preparation and restoration each have a 15-second recovery deadline.
The watchdog deadline for sleep includes the requested duration and 15 seconds for recovery.
The watchdog runs during sleep. Its expiry resets the main system and preserves the RTC domain for diagnosis.
The coordinator disables the watchdog after the experiment.
This backend rejects durations longer than one hour.

This path requires physical validation before normal use.
The installed RTOS's [automatic multicore hook][rtos-sleep] can stall another core while that core holds a required lock.
This implementation avoids that hook and waits for the cooperative stop before it stalls core 1.
Neither a build nor simulator success establishes wake reliability, sleep residency, or battery savings.
Physical checks must cover timer, BOOT, Function, touch, and Home wake events, including held and simultaneous inputs.
They must also cover repeated cycles, USB reconnection, display retention, and GNSS recovery.

### External power measurement

The BQ27220 observes battery current. It does not measure system current from the charger input.
The BQ25896 exposes voltage and battery charge-current measurements, but no total input-current measurement.
Its charge-current ADC uses 50 mA steps. See [the charger datasheet][charger], section 9.2.8 and register REG12.
An external USB meter can observe input power during sleep without a processor wake.
That reading includes battery charging, conversion losses, and other board loads.
After charge termination, an awake-to-sleep comparison reduces the contribution from battery charging.
The meter must resolve the expected low current. This experiment does not change charger settings.

## Verification limits

Native tests cover register decoding, signs, units, invalid values, readiness, presence changes, fault reads, and I2C failures.
Simulator scenarios cover display state and observation expiry.
Firmware builds verify target compatibility. These checks do not establish physical measurement accuracy.

Physical verification must compare battery voltage and current against independent measurements.
It must also cover USB insertion, USB removal, charge completion, battery absence, and shared-bus access during a display refresh.
Cell profile verification and calibration require separate hardware evidence.
Physical operation and measurement accuracy remain unverified.

The source review date is 2026-10-18.
The gauge reference is SLUUBD4A, revised November 2022.
The charger reference is SLUSC76C, revised May 2018.

[schematic]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/hardware/T5%20E-paper%20S3%20Pro%20V1.0%2024-12-24.pdf
[pinmap]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/docs/pinmap.md
[gauge]: https://www.ti.com/lit/ug/sluubd4/sluubd4.pdf
[charger]: https://www.ti.com/lit/ds/symlink/bq25896.pdf
[rtos-sleep]: https://github.com/esp-rs/esp-hal/blob/63e97be553f435f29171f7ae5f37a41ad737a75a/esp-rtos/src/sleep.rs
