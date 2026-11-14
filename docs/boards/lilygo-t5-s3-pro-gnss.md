# LILYGO GNSS adapter

[Backend profile](lilygo-t5-s3-pro.md)

This document describes firmware integration of the [portable GNSS receiver](../adapters/gnss.md).
The [location contract](../services/location.md) defines its portable observations.

## Configuration

The firmware [entry point](../../firmware/src/main.rs) supplies separate probe, recovery, and observation settings.
The [runtime configuration](../../firmware/src/gnss_worker.rs) contains ordered baud candidates, a per-candidate deadline, and a retry delay.
The [decoder configuration](../../crates/gnss/src/config.rs) contains only the observation silence limit.
Rates and durations use nonzero types. The probe requires a first candidate.

The [startup probe](../../crates/gnss/src/probe.rs) opens a fresh serial session for each candidate.
Only a complete sentence with a valid checksum selects a rate. This establishes communication, not receiver identity or a satellite fix.
Each probe is bounded. Firmware retries unsuccessful startup probes after its retry delay.
A successful rate remains in runtime state across suspend and receive retries.
Firmware sends no command to change the receiver's baud.

Silence invalidates observations and ends the receive session. Recovery opens another session at the retained rate.
It does not restart baud probing. Receiver reconfiguration can invalidate that retained choice and currently requires a firmware restart.
The probe is a separate helper. A future board configuration with a known baud can bypass it.

Large satellite storage remains static. The receiver borrows that storage and retains its observation configuration across reset.
NMEA framing is shared by probing and decoding. Each new session discards previous partial sentence state.

## Transport and power

The [firmware runtime](../platform/runtime.md#firmware-execution-placement) supplies the worker's interrupt-executor spawner.
The [runtime worker](../../firmware/src/gnss_worker.rs) composes GNSS with the [serial session contract](../platform/serial.md).
Firmware supplies UART resources, pins, and hardware settings to the [ESP serial adapter](../adapters/serial.md).
GNSS receives only serial sessions and does not configure ESP peripherals.

The [acquisition loop](../../crates/gnss/src/acquisition.rs) preserves a pending read across observation deadlines.
Read cancellation is permitted only when ending the session, including suspend or silence.
The runtime closes the session before serial sleep preparation. It invalidates receiver observations before acknowledging the pause.
ESP idle-transmitter handling remains in the serial owner.

Firmware owns the [shared auxiliary rail](lilygo-t5-s3-pro-power.md#auxiliary-rail), which also powers LoRa.
Startup admission remains blocked until rail initialization succeeds.
The rail coordinator awaits preparation before cutting power and restores power before releasing acquisition.
The retained baud survives these transitions. Location and UTC observations do not.

The worker remains a concrete Embassy task without heap allocation for sessions or startup probing.
The runtime constructs async UART sessions on the selected executor.

## Receiver variants and verification

The vendor supplies L76K and MIA-M10Q receivers with different default baud rates.
Its [GNSS example][gnss] probes 9600 and 38400 baud and controls the shared auxiliary supply.
This firmware uses a passive NMEA probe. It does not send receiver-specific configuration commands.

Hardware checks established receiver communication and satellite signal reports.
Satellite fix acquisition, time to first fix, and measurement accuracy are unverified.
The portable crate's [decoding and verification rules](../adapters/gnss.md) apply to both supported variants.

[gnss]: https://github.com/Xinyuan-LilyGO/T5S3-4.7-e-paper-PRO/blob/938d6189db0fde9a12558629bc1405bb49e1be78/examples/GPS/main/main.ino#L80-L96
