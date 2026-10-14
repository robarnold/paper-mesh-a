# Hardware backends

The [application architecture](architecture.md), [service contracts](device-interfaces.md),
and [display contract](display-constraints.md) define portable behavior.
Board profiles map that behavior to particular hardware.
They record evidence, limits, and implementation status.
Application components use capabilities and service results.
They do not select behavior by board name or chip model.

| Backend profile | Contents | Status |
| --- | --- | --- |
| [LILYGO T5 S3 E-Paper Pro](boards/lilygo-t5-s3-pro.md) | H752-01 inventory, controls, wiring, power limits, wake limits, service mapping, and initial hardware checks | Initial firmware target. Peripheral drivers stay planned work. |
| [LILYGO display evidence](boards/lilygo-t5-s3-pro-display.md) | Panel timing, scan geometry, reference-driver behavior, and numerical examples | Research and calibration plan. No measured Rust-driver performance. |

The [native simulator](simulator.md) implements the same service contracts.
Its current display preset uses the first board's geometry.
Future scenarios can use other capability combinations without emulation of a
physical board. A board conformance preset also models that profile's constraints.
The preset does not define the application architecture.

## Adding a backend

Record the board revision and sources.
Map available services and controls to portable contracts.
Distinguish absent hardware from missing drivers.
Keep wiring, resource dependencies, chip configuration, boot modes, and display
cost models in the backend profile and adapters.

Check the shared contract scenarios.
Add board conformance tests and hardware validation cases.
The intended design lets a second backend supply different capabilities and geometry without chip
checks in application components.
The [implementation status](architecture.md#scheduling-and-implementation-status)
records current fixed assumptions in the demonstration.

Vendor documents stay external.
The first target's [reference index](vendor/lilygo/t5-s3-pro/README.md) records
links at specified revisions and source metadata.
Backend profiles contain project findings in the project's own words.
