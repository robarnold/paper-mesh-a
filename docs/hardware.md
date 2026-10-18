# Hardware backends

The [software architecture](architecture.md), [service contracts](device-interfaces.md),
and [display contract](display-constraints.md) define portable behavior.
Board profiles map that behavior to particular hardware.
They record evidence, limits, and implementation status.
Application components use capabilities and service results.
They do not select behavior by board name or chip model.

| Backend profile | Contents |
| --- | --- |
| [LILYGO T5 S3 E-Paper Pro](boards/lilygo-t5-s3-pro.md) | Board inventory, resource mapping, adapter topics, and conformance cases |

Each profile records implementation status and links to detailed adapter documents.

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
The [application layout](application.md#layout-and-input) records current viewport requirements.
The [simulator execution](simulator/execution.md) records its current geometry.

Vendor documents stay external.
The first target's [reference index](vendor/lilygo/t5-s3-pro/README.md) records
links at specified revisions and source metadata.
Backend profiles contain project findings in the project's own words.
