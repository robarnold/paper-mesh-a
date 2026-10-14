# Portable e-paper display contract

This design defines backend requirements for shared components and display
policy. It specifies no panel, scan geometry, waveform implementation, or update
rate. The first target's [display evidence](boards/lilygo-t5-s3-pro-display.md)
provides calculations and reference-driver examples that support these boundaries.
The proposed `Display` extensions stay unimplemented.

## Capabilities and geometry

A backend supplies stable native geometry, mounting information, and supported
rendering and update capabilities.
The [shared orientation state](architecture.md#orientation-state-and-coordinate-spaces)
derives the logical viewport for components.
The backend controls native framebuffer size and storage order.
Components must not contain fixed panel dimensions.

| Capability | Required information for shared policy |
| --- | --- |
| Rendering | Supported logical shades and formats, with mode restrictions |
| Update regions | Whether damage can limit physical work, with region alignment requirements |
| Quality modes | Available quality and latency choices, with required cleanup or history |
| Concurrency | Serialized updates or independently active regions |
| Cancellation | Whether the backend can cancel or replace accepted work, or only combine work before submission |
| Presentation state | Completion identity, content retention guarantees, and recovery requirements for repaint |
| Cost estimate | Optional latency and energy estimates, with uncertainty, assumptions, and validity |

A chip name or enum value does not establish a capability.
The backend must reject unsupported operations or use an explicit alternative.
The current `Gray4` contract selects a logical rendering format.
It does not establish identical quality for all modes or specify buffer packing.

## Damage, scheduling, and cost

Components report logical damage, urgency, and quality intent.
The compositor combines damage.
It restores old regions when content moves or disappears.
It renders the latest state without a queue of all intermediate animation frames.
The orientation adapter converts damage into backend coordinates.
It uses the same snapshot as drawing.

The backend interprets damage through its update model.
Changed-pixel count alone cannot predict cost for all backends.
A backend can have row, rectangle, tile, or full-frame costs.
Some backends can shorten work for unchanged regions.
Shared policy can use estimates without scan clocks, line widths, or waveform
phases. Each optional estimate must identify its measured or synthetic basis.

The intended backend reports preparation, submission, first visible change,
and settled completion where observable.
Scan rate is not application frame rate.
A small region does not necessarily settle proportionally faster.
Backend reports and measurements establish useful limits.
The architecture specifies no fixed pixel-count-to-time formula.

## Component behavior

| Owner | Responsibility |
| --- | --- |
| UI components | Stable layout and background where practical, logical damage, useful update intervals |
| Shared compositor | Combined damage, retained rendering state, bounded pending work |
| Shared orientation policy | Consistent snapshots for layout, drawing, input, submitted frames, and presented frames |
| Shared display policy | Supported mode selection, combined work, cleanup that obeys capabilities |
| Backend adapter | Pixel packing, transfer and drive operations, history, power sequences, completion, failures |

The intended selection markers invalidate bounded regions.
The intended value updates prevent movement of adjacent content when that movement is not necessary.
Clocks and progress indicators must use useful update intervals.
Blinking and animation must have explicit resource budgets.
When presentation is slow, scrolling combines movement.
Drawing input updates application state promptly, even when presentation is late.

Layout stays a usability decision.
A panel's row direction does not determine every component layout.
Interactive feedback can request low latency.
Settled content can request high quality.
The backend checks each requested mode and reports the accepted mode.

Cleanup frequency depends on driver history and measured behavior.
The contract gives no universal interval for full-screen flashes.

## Lifecycle and ownership

Desired content, accepted work, and completed presentation are separate states.
Successful submission confirms acceptance, not visibility.
Physical presentation can change progressively.
A completed target does not imply an atomic optical replacement.

Input must use the visible layout's orientation, as the architecture specifies.
Input stays blocked while presentation is unknown during recovery.

Reset, transfer failure, or lost driver history can invalidate differential
updates. Visible content can stay despite that invalidation.
The backend defines recovery to a known state.
It also defines content retention for each lifecycle transition.
A buffer clear is a software operation.
It does not imply a physical refresh.

The backend owns buffers and keeps accepted transfer data valid until completion.
This requirement includes DMA and work on another core.
Applications must obey buffer ownership and backpressure rules.
They must not assume a particular framebuffer count.

The current `refresh()`/`is_busy()` interface serializes an update.
It does not represent damage, mode selection, completion identity, or asynchronous
failure. The demonstration redraws its complete buffer.
Future consumers will drive extensions to these limits.
Components must not generate electrical drive commands.

## Simulator and hardware validation

Portable scenarios must include supported and unsupported region updates, different cost
models, changed geometry, and changed mounting.
They must also include rapid input while busy, combined submissions, faults, and idle
periods without display work.
Components must stay correct without estimates or a requested fast mode.
Scenarios use deterministic virtual time and identified synthetic costs.

Board calibration measures preparation, transfer, first visible change, settling,
quality, ghosting, temperature effects, and energy separately.
Records include driver revision, configuration revision, and history.
Results supply backend capabilities and estimates.
They do not become fixed values in application components.
The [first backend's calibration plan](boards/lilygo-t5-s3-pro-display.md#simulator-and-hardware-validation)
includes its row and scan measurements.
