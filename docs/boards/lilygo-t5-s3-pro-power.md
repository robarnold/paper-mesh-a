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
Charge control, power removal, and sleep remain planned work.

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
