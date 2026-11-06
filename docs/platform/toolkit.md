# Application toolkit

[Platform overview](../platform.md)

Applications supply control identities, actions, and geometry so the toolkit can serve different screens and devices.
The [tap recognizer](../../crates/platform/src/input.rs) returns the activated control on release.
Use the [shared control rules](../ui.md#control-and-data-states) when connecting hit tests to actions.

The [system navigation router](../services/input.md#system-navigation-routing) consumes the Home key before application bindings.
It has no application dependency or screen layout.

## User interface components

Buttons and navigation cards use the same bounds and enabled state for drawing and hit testing.
Retained controls publish input bindings to shared layers. The caller supplies callbacks and control data.
Badges annotate content and have no input behavior.
The [text input contract](text-input.md) defines how the field receives edits from the soft keyboard.

The caller must allocate enough space for essential text. Header and control renderers clip to their assigned regions.
Compare working controls in the [visual guide](../simulator/execution.md#ui-guide).
See the [toolkit API](../../crates/platform/src/ui/mod.rs) for available components.

## Pages and shells

The [UI guide](../simulator/execution.md#ui-guide) demonstrates these contracts with a complete example application.

`Page` supplies retained elements, metadata, lifecycle hooks, and callback bindings.
Its associated `Id` and control data types belong to the application.
`PageRuntime` prepares layout, publishes elements, and dispatches callbacks after layer traversal.
Pages provide `did_enter`, `will_leave`, `sync_elements`, and `configure_handlers` hooks.
Callbacks receive mutable page state, a `UiContext`, and typed control data.
Custom graphics use `draw_custom` within the layer clip.
See [UI](ui.md) for ownership, publication, and input rules.
`PageInfo` supplies the destination, title, context, presentation, and navigation availability.
It contains no parent or return destination.
`PageEvent` reports no change, a visible change, or a navigation request.

`PageSet` dispatches by identity to application-defined page storage.
Its dispatch methods delegate to page metadata and the common `PageRuntime` operations.
The collection routes layout preparation, invalidation, focus, and callbacks to the selected page.
The collection can retain shared service state across related destinations.
A single Page also implements PageSet without an adapter.

`Navigator<Pages, CAPACITY>` owns the shared layers, shell, page collection, system keyboard, focus, and bounded return stack.
`Navigator::launch(home, shell, pages)` enters the root exactly once before input or rendering.
It does not require a heap, global state, an executor, or trait objects.
The application selects the history capacity through the const parameter.

For static storage, `Navigator::unlaunched` supplies a constant initializer without page entry.
The runtime then calls `launch_in_place` once before input, navigation, or rendering.
This avoids a large temporary return value on an embedded stack.
Ordinary callers can continue to use `Navigator::launch`.

`Shell` supplies the shared frame and Back region. The layer stack recognizes its tap.
`ShellLayout` supplies viewport, content, header, divider, and Back regions in logical coordinates.
It contains no backend dimensions or resource controls.
The caller must supply non-overlapping regions within the viewport and sufficient space for readable text.
Clipping does not translate the coordinate origin.

The application integrates these contracts in this order:

1. Define retained page storage and its PageSet dispatch.
2. Construct a Shell with the application layout.
3. Call `Navigator::launch` with the root identity, shell, and page collection.
4. Give logical touch input to `Navigator::touch`.
5. Pass external navigation requests to `Navigator::navigate`.
6. Mark visible updates and returned transitions for display submission.
7. Call `Navigator::prepare` after service updates and before drawing.
8. Render through `Navigator::render` with the application style.
9. Call `Navigator::submitted` after successful refresh acceptance, then `completed` after presentation.

The layer store records the submitted revision. Completion acknowledges that revision even after navigation.
Only one frame can await completion. The display backend enforces this limit.
Call `invalidate` after display failure or recovery before a replacement frame.
Entry invalidates the destination elements. Input uses committed layers immediately, including before the first completed frame.

The layer stack routes Back and page input through the same hierarchy.
It handles page navigation requests and completes exit and entry callbacks before returning an update.
The application does not apply those callbacks separately.
`pages` and `pages_mut` provide access for service polling and retained page state.
`history()` exposes the complete return stack as a read-only slice, with the oldest caller first.
It excludes the current page. Its last entry matches `return_to()`.

`Navigation::Open(id)` records the current destination, then opens the requested page.
`Navigation::Back` removes the latest return destination and returns to it.
`Navigation::Home` returns to the configured root and clears the return stack.
Opening the root has the same effect as Home.
Opening the current page does not add history or repeat lifecycle callbacks.
Back at the root has no effect. Repeated visits to other destinations retain their invocation order.

Disabled navigation rejects requests without changing the destination, history, or lifecycle.
A full return stack rejects Open with `NavigationError::HistoryFull` and leaves the current page active.
Back and Home remain available when the stack is full, subject to page restrictions.
Metadata must match the requested identity. The page collection must cover every valid destination.

A transition cancels the old page's input and the shell's armed Back gesture.
The navigator suppresses existing contacts until they all lift or the input session reports cancellation.
The runtime also suppresses held contacts across system navigation through the input session.
When the runtime suppresses physical releases, it must deliver `TouchEvent::Cancel` before the navigation request.

The navigator renders the active page through its shell.
System layers clear the viewport and clip standard content. The shell draws the header above the content.
Full-screen pages receive the complete viewport without a header.
The application and display service still control dirty state, refresh submission, and physical completion.

The [UI layout engine](layout.md) supplies rows, columns, grids, separate input regions, and logical drawing damage.
Overlay composition, modal input capture, and scrolling remain planned work.
The [UI requirements](ui-requirements.md) define their consumers, boundaries, and implementation priorities.

## Time and display helpers

`PeriodicTimer` keeps its original phase after a late poll.
It reports at most one event per poll.
Its interval must be greater than zero.

`ObservationWindow` rejects future timestamps and observations at or above the maximum age.
It also calculates the next expiry from supplied receipt times.
Deadline addition saturates at the clock limit.

The formatters use bounded strings without heap allocation.
Function names specify display units, such as `speed_kmh` and `elevation_meters`.
Missing values produce unavailable markers.
The application selects a formatter and decides when a value is usable.

`display::present` renders only when the caller requests a frame and the display is ready.
It reports success after refresh submission.
The caller keeps its redraw request after an error or a deferred submission.
The backend continues to control physical completion and driver recovery.
