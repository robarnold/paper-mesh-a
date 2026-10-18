# Platform

The platform supplies common functionality and a toolkit for application development.
Applications combine toolkit components into screens and workflows.
The platform also supplies services, adapters, and execution support.
The [architecture](architecture.md) defines the application, platform, and backend boundaries.

## Responsibilities

The platform groups functionality by responsibility.
These groups apply across applications and backends.
These groups do not require a module for each feature.

| Area | Responsibility |
| --- | --- |
| Application toolkit | Reusable components for application behavior and presentation |
| Service contracts | Portable requests, observations, capabilities, ownership, and failure results |
| Shared state and policy | Consistent behavior across components and service requests |
| Execution and resources | Event delivery, deadlines, lifecycle control, and resource coordination |
| Backend integration | Adapters between portable contracts and hardware or simulation |

## Application boundary

Applications select their content, screens, workflows, layout, actions, and policy values.
The platform supplies common mechanisms that accept these choices.
Reusable components have no dependency on a particular application's screens or state.
The current application is one consumer of the toolkit.

A function belongs to the platform when applications can share its behavior or use its portable contract.
Application choices remain explicit inputs to that function.
The platform can supply components before a second application needs them.
The platform does not contain application actions or fixed application layouts.

## Backend boundary

Backends implement portable contracts for hardware or simulation.
They control native resources and convert device details into portable results.
Runtime code connects an application to these services.
Application code uses contracts and capabilities without board or chip checks.

The platform crate contains reusable components and portable contracts.
The platform also includes runtime code and backend implementations outside that crate.
The [architecture](architecture.md#boundaries) maps these responsibilities to code.

## Detailed topics

Each topic document identifies current behavior and planned work separately.
The platform overview does not imply that all services are implemented.

| Topic | Contents |
| --- | --- |
| [Application toolkit](platform/toolkit.md) | Current components, caller choices, and API behavior |
| [Runtime execution](platform/runtime.md) | Implementation boundaries and the current application loop |
| [Orientation state and coordinate spaces](platform/orientation.md) | Coordinate transforms, orientation policy, and frame state |
| [Display lifecycle and ownership](platform/display.md) | Frame submission, completion, buffers, and recovery boundaries |
| [Device services](device-interfaces.md) | Implemented interfaces and planned portable service contracts |
| [Display contract](display-constraints.md) | Component requirements and planned display services |
| [Hardware backends](hardware.md) | Board profiles and hardware details |
| [Simulator](simulator.md) | Native backend behavior and deterministic scenarios |

The [documentation principles](../DOCUMENTATION.md#documentation-principles) specify how to maintain this structure.
