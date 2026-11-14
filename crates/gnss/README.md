# GNSS receiver

This `no_std` crate decodes NMEA into platform location, motion, UTC, and satellite observations.
It provides optional startup baud probing and acquisition over platform serial sessions.

Callers own receiver storage and supply monotonic time, deadline futures, and observation publication.
The crate uses Embassy future combinators without requiring an Embassy executor or timer driver.
It has no ESP, UART-peripheral, GPIO, shared-rail, or system-sleep dependencies.

Firmware selects hardware, probe candidates, timing policy, and executor placement.
Its runtime closes sessions and prepares hardware before acknowledging suspend.
The [receiver documentation](../../docs/adapters/gnss.md) describes decoding, acquisition, and verification limits.
