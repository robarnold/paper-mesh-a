# Hardware backends

The [application architecture](architecture.md), [service contracts](device-interfaces.md),
and [display contract](display-constraints.md) define portable behavior. Board
profiles map that behavior to particular hardware and record evidence, limits,
and implementation status. Application components consume capabilities and
service results; they do not select behavior by board name or chip model.

| Backend profile | Contents | Status |
| --- | --- | --- |
| [LILYGO T5 S3 E-Paper Pro](boards/lilygo-t5-s3-pro.md) | H752-01 inventory, controls, wiring, power/wake restrictions, service mapping, and bring-up checks | Initial firmware target; peripheral drivers remain planned |
| [LILYGO display evidence](boards/lilygo-t5-s3-pro-display.md) | Panel timing, scan geometry, reference-driver behavior, and numerical examples | Research and calibration plan; no measured Rust-driver performance |

The [native simulator](simulator.md) implements the same service contracts. Its
current display preset follows the first board's geometry. Future scenarios can
exercise other capability combinations without claiming to emulate a physical
board. A board-specific conformance preset additionally models that profile's
constraints; it is not the definition of the application architecture.

## Adding a backend

Record the board revision and sources, map available services and controls to
portable contracts, and distinguish absent hardware from missing drivers. Keep
wiring, shared-resource dependencies, chip configuration, boot modes, and
native display cost models in the backend profile and adapters.

Check the shared contract scenarios, then add board-specific conformance and
hardware validation cases. A second backend should supply a different capability
set and geometry without new chip-name conditionals in application components.
Current hard-coded demo assumptions are tracked in the
[architecture's implementation status](architecture.md#scheduling-and-implementation-status).

Vendor documents remain external. The first target's
[reference index](vendor/lilygo/t5-s3-pro/README.md) records pinned links and source
metadata; our profiles contain findings in our own words.
