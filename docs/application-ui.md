# Application UI rules

This document defines choices for the current demonstration application.
The [shared UI standard](ui.md) defines common navigation, controls, visual roles, and validation requirements.
The [application guide](application.md) defines feature behavior, measurements, and time limits.

## Integration and ownership

The application owns one platform `Navigator<Pages, 8>` with a shell and retained pages without heap allocation.
[`pages.rs`](../crates/app/src/pages.rs) defines destination identities, page storage, and Page adapters.
The history capacity permits eight return destinations. Current workflows require at most four.
[`screen.rs`](../crates/app/src/screen.rs) selects `Style::GRAYSCALE` and supplies portrait geometry.
The navigator owns the current destination and return history. Page visibility flags control service polling and compatibility accessors, not routing.

The navigator launches the shell and runs page entry and exit callbacks.
The application page collection uses these callbacks to coordinate shared services.
It keeps Power observations active while a power subpage is visible.
It closes service controllers when their destination group is no longer active.
The runtime connects reserved hardware Home requests to the navigator through `App::go_home`.

## Destinations and operation restrictions

| Page | Usual caller | Presentation |
| --- | --- | --- |
| Home | Application launch | Standard |
| Diagnostics | Home | Standard |
| Display | Diagnostics | Standard, or full-screen during a solid pattern |
| Touch | Diagnostics | Standard |
| Radio | Diagnostics | Standard |
| GNSS | Diagnostics | Standard |
| Power | Diagnostics | Standard |
| Suspend | Power | Standard |
| Power off | Power, or the active page through the hardware shortcut | Standard |
| USB touchpad | Home | Full-screen |

These callers describe current workflows. They are not page metadata.
Back and Home follow the [shared navigation rules](ui.md#navigation-and-presentation).
After a refused hardware shutdown, Back returns to the page where the user invoked shutdown.
The Diagnostics menu groups component checks and measurements.
USB touchpad remains a separate destination on Home.

Suspend and Power off block navigation while their requests are pending.
A failed request restores navigation and permits a new request.
Home does not restart the application or reset the tap counter.
A page change cancels an active radio scan and incomplete gestures.
Reports remain available according to the feature rules in the application guide.

Display patterns use the complete viewport and ignore touch input.
Function returns from a pattern to its caller, normally Diagnostics. Home returns directly to Home.
Either exit requests a complete refresh after the active display operation finishes.
USB touchpad forwards contacts to the host. Function and Home return to Home.

## Portrait layout

The application uses a 540 × 960 logical viewport.
The runtime connects this viewport to the backend through the portrait adapter.
The selected shared style profile defines text roles, grayscale shades, and spacing values.

| Element | Application geometry |
| --- | --- |
| Horizontal content margins | 28 pixels |
| Context region | x = 28, y = 24, width = 368, height = 12 |
| Title region | x = 28, y = 48, width = 368, height = 24 |
| Back control | x = 408, y = 24, width = 104, height = 56 |
| Header divider | y = 92 |
| Content region | x = 28, y = 104, width = 484, height = 840 |

The title region leaves the selected control gap before Back.
Individual pages define content placement within the content region.
Page renderers select semantic text and color roles from the application style.
Display test patterns and the grayscale sample retain their exact diagnostic shades.

## Validation

Exercise every destination through the platform navigator.
Check each caller return, direct Home navigation, pending request, and full-screen exit.
Check retained counts, reports, selected power tabs, and radio cancellation after page changes.
Review the application screenshots alongside the shared visual catalog.
The [shared checklist](ui.md#display-updates-and-validation) defines the remaining UI checks.
