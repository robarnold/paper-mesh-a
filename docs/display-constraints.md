# Portable e-paper display contract

This design describes what shared components and display policy need from a
backend. It does not prescribe a panel, scan geometry, waveform implementation,
or update rate. The first target's [display evidence](boards/lilygo-t5-s3-pro-display.md)
provides the concrete measurements and reference-driver examples that motivated
these boundaries. Proposed extensions below are not yet implemented by `Display`.

## Capabilities and geometry

A backend supplies stable native geometry, mounting information, and supported
rendering/update capabilities. The [shared orientation state](architecture.md#orientation-state-and-coordinate-spaces)
derives the logical viewport used by components. Native framebuffer size and
storage order remain backend concerns; components do not hard-code panel dimensions.

| Capability | What shared policy needs to know |
| --- | --- |
| Rendering | Supported logical shades/formats and any mode-specific restrictions |
| Update regions | Whether damage can limit physical work, and any region alignment constraints |
| Quality modes | Available quality/latency choices and required cleanup or history |
| Concurrency | Whole-update serialization or independently active regions |
| Cancellation | Whether work can be canceled, replaced, or only coalesced before submission |
| Presentation state | Completion identity, retained-content guarantees, and whether recovery requires repainting |
| Cost estimate | Optional expected latency/energy with uncertainty, assumptions, and validity |

Do not infer a capability from a chip name or enum value. Unsupported operations
must be rejected or handled by an explicit fallback. The current `Gray4` contract
selects a logical rendering format; it does not establish identical quality in
every backend mode or dictate buffer packing.

## Damage, scheduling, and cost

Components report logical damage and urgency/quality intent. The compositor
combines damage, restores old regions when content moves or disappears, and
renders the latest state instead of queuing every intermediate animation frame.
The orientation adapter converts damage to backend coordinates using the same
snapshot as drawing.

The backend interprets that damage using its actual update model. Changed-pixel
count alone is not a universal cost estimate. Different backends can have
row-oriented, rectangular-region, tile, or full-frame costs; some can shorten
unchanged work and some cannot. Shared policy can use a backend estimate without
knowing scan clocks, line widths, or waveform phases. Estimates are optional and
must state whether they are measured or synthetic.

A backend should distinguish preparation, submission, first visible change, and
settled completion where observable. A scan rate is not an application frame
rate, and a small region need not settle proportionally faster. Backend reports
and measurements establish useful limits; the architecture supplies no fixed
pixels-to-time formula.

## Component behavior

| Owner | Responsibility |
| --- | --- |
| UI components | Stable layout/background where practical, logical damage, meaningful update cadence |
| Shared compositor | Damage merging, retained rendering state, bounded pending work |
| Shared orientation policy | Matching snapshots for layout, drawing, input, and submitted/presented frames |
| Shared display policy | Select advertised modes, coalesce work, schedule cleanup according to capabilities |
| Backend adapter | Pixel packing, transfer/drive mechanics, history, power sequencing, and completion/failure |

Selection markers should invalidate bounded regions. Updating a value should
avoid moving neighbors unnecessarily. Clocks and progress indicators use a useful
cadence; blinking and animation must have deliberate budgets. Scrolling coalesces
movement when presentation lags. Drawing input updates application state promptly
while the visible result catches up. Layout remains a usability choice, not a
rule derived from one panel's row direction.

Interactive feedback can request latency preference and settled content can
request quality preference. The backend must validate the requested mode and
state what it accepted. Cleanup frequency depends on its history and measured
behavior, not a universal periodic full-screen flash.

## Lifecycle and ownership

Keep desired content, accepted work, and last completed presentation distinct.
Successful submission means acceptance, not visibility. Physical presentation
may change progressively; a completed target does not imply an atomic optical
swap. Input must use the orientation of the visible layout, as described in the
architecture, and remain blocked while presentation is unknown during recovery.

Reset, failed transfers, or lost driver history can invalidate differential
updates even if visible content is retained. A backend declares how to recover
known state and which content survives each lifecycle transition. Buffer clears
are software operations; they do not by themselves imply a physical refresh.

The backend owns buffers and keeps accepted transfer data valid until completion,
including work on another core or DMA. Applications must follow its ownership
and backpressure contract rather than assume a particular number of framebuffers.

Today `refresh()`/`is_busy()` serializes an update and does not express damage,
mode selection, identified completion, or asynchronous failures. The demo redraws
its whole buffer. Those are implementation limits to extend as consumers need
them; components should never need to generate electrical drive commands.

## Simulator and hardware validation

Portable scenarios should compare supported and unsupported region updates,
different cost models, changed geometry/mounting, rapid input while busy,
coalesced submissions, faults, and idle periods without display work. Verify that
components remain correct when an estimate is absent or a requested fast mode
is unavailable. Use deterministic virtual time and label synthetic costs.

Board-specific calibration separately measures preparation and transfer costs,
first visible change, settling, quality/ghosting, temperature sensitivity, and
energy. Record driver/configuration revision and history. Those results populate
backend capabilities and estimates; they do not become constants in application
components. The [first backend's calibration plan](boards/lilygo-t5-s3-pro-display.md#simulator-and-hardware-validation)
contains its row/scan-specific measurements.
