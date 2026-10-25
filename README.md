# paper-mesh-a

This project contains a Rust application and a platform toolkit for e-paper devices.
Applications use common components and portable services from the platform.
Hardware and simulator backends supply those services.
The first hardware backend targets the LILYGO T5 S3 E-Paper Pro with LoRa.

The [architecture](docs/architecture.md) defines code boundaries and dependencies.
The [application](docs/application.md) describes the current demonstration.
The [platform overview](docs/platform.md) describes common responsibilities and links to detailed topics.

## Start development

Run all project tools in the devcontainer.
The [development guide](docs/development.md) specifies host requirements and container setup.

```sh
devcontainer up --workspace-folder .
devcontainer exec --workspace-folder . just ci
devcontainer exec --workspace-folder . just sim
```

Open [localhost:8080](http://localhost:8080).
The [simulator guide](docs/simulator.md) describes controls, scenarios, and API access.
To stop the server, press Ctrl+C.

## Documentation

| Area | Entry point |
| --- | --- |
| Contribution rules | [Documentation guide](DOCUMENTATION.md) and [agent instructions](AGENTS.md) |
| Code organization | [Software architecture](docs/architecture.md) |
| Application behavior | [Current application](docs/application.md) |
| Common components and execution | [Platform](docs/platform.md) |
| Design guidelines and examples | [Visual UI guide](docs/simulator/execution.md#ui-guide) and [shared UI standard](docs/ui.md) |
| Portable contracts | [Device services](docs/device-interfaces.md) and [display contract](docs/display-constraints.md) |
| Simulation | [Native simulator](docs/simulator.md) |
| Hardware | [Backend profiles](docs/hardware.md) and [vendor references](docs/vendor/lilygo/t5-s3-pro/README.md) |
| Development procedures | [Environment and CI](docs/development.md) and [firmware builds](docs/firmware-build.md) |
| Project terms | [Terminology](docs/terminology.md) |

Each topic records its implemented behavior, planned work, and limitations.
The [display driver notice](crates/ed047tc1/README.md) records its source and GPL license.
