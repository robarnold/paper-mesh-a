# Project terminology

These terms identify software and hardware concepts in this project.
The [agent guidelines](../AGENTS.md#english-prose) specify ASD-STE100 for English
prose. This list supports the same terms for the same concepts. It does not replace the
standard's dictionary or permit other meanings and parts of speech.

## Technical nouns

| Term | Meaning |
| --- | --- |
| Adapter | Code that connects a service contract to another interface or coordinate space |
| Application | Shared state, behavior, and rendering in `crates/app` |
| Availability | Current readiness or fault state of a service |
| Backend | Implementation of platform services for hardware or the simulator |
| BSSID | Address that identifies a Wi-Fi access point in discovery results |
| Backpressure | A limit on new requests when a service cannot accept more work |
| Board profile | Backend description of fitted hardware, wiring, calibration, and resource dependencies |
| Capability | Supported service operation or limit |
| Compositor | Shared component that combines damage and prepares display content |
| Contract | Required behavior at a service boundary, including ownership and failure results |
| Damage | Region whose target image differs from the last application image |
| Devcontainer | Container that contains and runs the project's development tools |
| Discovery | Radio operation that finds Wi-Fi access points or Bluetooth LE devices |
| Epoch | Time origin for one application session |
| Framebuffer | Memory that stores pixel values for a frame |
| Gesture | Input action that shared policy derives from a sequence of control or contact events |
| Logical viewport | Display area and coordinate space that the application uses |
| Mounting transform | Fixed rotation from native panel coordinates to device-body coordinates |
| Native coordinates | Coordinates that use the display backend's storage orientation |
| Observation | Service value with units, validity, and an observation time |
| Platform | Resources and execution environment that supply application services |
| Presented snapshot | State associated with the last completed display frame |
| Resource coordinator | Backend owner that controls access to shared buses, interrupts, and power domains |
| RSSI | Received signal strength indicator, expressed in dBm for discovery results |
| Radio debugger | Shared screen that shows discovery results and scan controls |
| Scenario profile | Simulator configuration for capabilities, timing, and faults |
| SSID | Name that identifies a Wi-Fi network |
| Staging buffer | Pixel storage that accepts drawing before refresh submission |
| Waveform | Sequence of electrical drive commands that changes an e-paper pixel's shade |

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
