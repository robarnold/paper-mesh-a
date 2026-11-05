# Application UI rules

This document defines choices for the current demonstration application.
The [shared UI standard](ui.md) defines common navigation, controls, visual roles, and validation requirements.
The [application guide](application.md) defines feature behavior, measurements, and time limits.

## Integration and ownership

The application retains pages so navigation can preserve measurements and service state.
The [page collection](../crates/app/src/pages.rs) defines destinations and storage.
The platform navigator owns the current destination and its return history.

The navigator launches the shell and runs page entry and exit callbacks.
The application page collection uses these callbacks to coordinate shared services.
It keeps Power observations active while a power subpage is visible.
It closes service controllers when their destination group is no longer active.
The runtime connects reserved hardware Home requests to the navigator through `App::go_home`.

## Destinations and operation restrictions

Back follows the caller, including when a hardware shortcut opens a page from outside its usual workflow.
See the [shared navigation rules](ui.md#navigation-and-presentation).
After a refused hardware shutdown, Back returns to the page where the user invoked shutdown.
The Diagnostics menu groups component checks and measurements.
USB touchpad remains a separate destination on Home.
Calculator is also a Home destination. Its retained state survives Back and Home.
Its keypad uses platform layout constraints and separate input outsets. See the [UI contract](platform/ui.md).

Suspend and Power off block navigation while their requests are pending.
A failed request restores navigation and permits a new request.
Home does not restart the application or reset the tap counter.
Leaving the radio debugger follows its [scan cancellation policy](application.md#radio-debugger).
Reports remain available according to the feature rules in the application guide.

Display patterns use the complete viewport and ignore touch input.
Function returns from a pattern to its caller, normally Diagnostics. Home returns directly to Home.
Either exit requests a complete refresh after the active display operation finishes.
USB touchpad forwards contacts to the host. Function and Home return to Home.

## Portrait layout

The [screen layout](../crates/app/src/screen.rs) reserves separate space for the title and Back control so either can change without overlap.
The portrait adapter maps that logical layout to the backend.
Individual pages define content placement within the content region.
Page renderers select semantic text and color roles from the application style.
Display test patterns and the grayscale sample retain their exact diagnostic shades.

## Validation

Exercise every destination through the platform navigator.
Check each caller return, direct Home navigation, pending request, and full-screen exit.
Check retained counts, reports, selected power tabs, and radio cancellation after page changes.
Review the application screenshots alongside the shared visual catalog.
The [shared checklist](ui.md#display-updates-and-validation) defines the remaining UI checks.
