# Project terminology

These terms identify software and hardware concepts in this project.
The [documentation guide](../DOCUMENTATION.md#english-prose) specifies ASD-STE100 for English
prose. This list supports the same terms for the same concepts. It does not replace the
standard's dictionary or permit other meanings and parts of speech.

## Technical nouns

| Term | Meaning |
| --- | --- |
| Adapter | Code that connects a service contract to another interface or coordinate space |
| Application | Screens, workflows, state, layout, actions, and policy choices for a specific use |
| Application toolkit | Reusable components and services that applications combine into screens and workflows |
| Availability | Current readiness or fault state of a service |
| Backend | Implementation of platform services for hardware or the simulator |
| BSSID | Address that identifies a Wi-Fi access point in discovery results |
| Backpressure | A limit on new requests when a service cannot accept more work |
| Board profile | Backend description of fitted hardware, wiring, calibration, and resource dependencies |
| Capability | Supported service operation or limit |
| C/N0 | Carrier-to-noise density ratio, expressed in dB-Hz for GNSS signals |
| Compositor | Shared component that combines damage and prepares display content |
| Contract | Required behavior at a service boundary, including ownership and failure results |
| Course | Direction of travel over ground, measured clockwise from true north |
| Damage | Region whose target image differs from the last application image |
| Devcontainer | Container that contains and runs the project's development tools |
| Discovery | Radio operation that finds Wi-Fi access points or Bluetooth LE devices |
| Epoch | Time origin for one application session |
| Elevation | Vertical position above mean sea level for the GNSS readout |
| Framebuffer | Memory that stores pixel values for a frame |
| Gesture | Input action that shared policy derives from a sequence of control or contact events |
| GGA | NMEA message that supplies position, fix quality, and the number of satellites in the solution |
| GNSS | Global navigation satellite system |
| GSV | NMEA message that supplies satellites in view and their signal levels |
| Logical viewport | Display area and coordinate space that the application uses |
| Mounting transform | Fixed rotation from native panel coordinates to device-body coordinates |
| MSL | Mean sea level, the reference for GNSS elevation |
| Native coordinates | Coordinates that use the display backend's storage orientation |
| NMEA | Message protocol that the GNSS adapter decodes |
| Observation | Service value with units, validity, and an observation time |
| Page | One application destination with content, input behavior, and entry and exit callbacks |
| Panel | A related content region within a page; no separate platform API exists yet |
| Platform | Application toolkit, service contracts, adapters, resources, and execution |
| Platform crate | Reusable toolkit implementations and portable contracts in `crates/platform`, without an application dependency |
| Position fix | Geographic position that a location provider marks as valid |
| Presented snapshot | State associated with the last completed display frame |
| Resource coordinator | Backend owner that controls access to shared buses, interrupts, and power domains |
| RMC | NMEA message that supplies position status, speed, course, UTC, and date |
| RSSI | Received signal strength indicator, expressed in dBm for discovery results |
| Radio debugger | Application screen that shows discovery results and scan controls |
| Runtime | Code that connects an application to services and controls execution and event delivery |
| Scenario profile | Simulator configuration for capabilities, timing, and faults |
| Navigator | Platform owner of shell launch, return history, active destination, and page lifecycle |
| Shell | Persistent UI frame that supplies the shared header, Back gesture, and content bounds |
| Style role | Named visual purpose that resolves to a selected font, color, or spacing value |
| SSID | Name that identifies a Wi-Fi network |
| Staging buffer | Pixel storage that accepts drawing before refresh submission |
| UTC | Coordinated Universal Time |
| Waveform | Sequence of electrical drive commands that changes an e-paper pixel's shade |
| WGS84 | Geodetic reference system for GNSS latitude and longitude |

Established names such as Rust, Embassy, LoRa, and GT911 keep their official
spelling. API identifiers, register names, units, and protocol names keep their
specified form. A source title can differ from the preferred term in project prose.

## Technical verbs

Software procedures use established computing verbs such as compile, decode,
encode, initialize, render, serialize, and synchronize. Each verb identifies its
specific software operation. These verbs do not permit unrelated figurative uses.
For example, a driver can serialize requests, and a renderer can render a frame.

Descriptions distinguish capability from availability. They also distinguish
request acceptance from completion and current implementation from planned work.
