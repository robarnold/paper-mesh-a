# Application toolkit

[Platform overview](../platform.md)

This document describes the implemented toolkit components.

Common functionality belongs in the platform crate.
Reusable components accept caller choices through parameters and callbacks.
They have no dependency on application screens or state.
Toolkit modules group related functions by responsibility.

| Module | Current functionality | Caller choices |
| --- | --- | --- |
| [`ui`](../../crates/platform/src/ui/mod.rs) | Headers, buttons, navigation cards, text badges, and style | Bounds, text regions, labels, fonts, colors, and enabled state |
| [`ui::navigation`](../../crates/platform/src/ui/navigation.rs), [`ui::shell`](../../crates/platform/src/ui/shell.rs), [`ui::page`](../../crates/platform/src/ui/page.rs) | Shell launch, return history, input dispatch, page presentation, and lifecycle | Page identities, retained storage, geometry, and restrictions |
| [`ui::format`](../../crates/platform/src/ui/format.rs) | Bounded value labels and satellite labels | Values and format functions with explicit units |
| [`input`](../../crates/platform/src/input.rs) | Tap recognition and cancellation | Hit testing, control identity, and resulting action |
| [`input::navigation`](../../crates/platform/src/input/navigation.rs) | Reserved Home-key routing | Backend key identity and application home destination |
| [`time`](../../crates/platform/src/time.rs) | Periodic deadlines and observation age checks | Epoch, interval, current time, and maximum age |
| [`display`](../../crates/platform/src/display.rs) | Frame submission with busy and error checks | Redraw request and rendering callback |
| [`rotation`](../../crates/platform/src/rotation.rs) | Pixel and touch coordinate conversion | Native dimensions |

`TapGesture` returns a control identity after one contact stays on that control through release.
Movement outside the control cancels the tap. Movement back inside does not arm it again.
A second contact cancels the tap until all contacts lift.
Raw contact events remain available to other consumers.
Release and cancellation clear the armed control.
The application maps the returned identity to an action.

The [system navigation router](../services/input.md#system-navigation-routing) consumes the Home key before application bindings.
It has no application dependency or screen layout.

## User interface components

The [shared UI standard](../ui.md) defines visual roles and interaction requirements.
`Style` supplies fonts, color roles, and a `Spacing` profile.
`Style::text` resolves a semantic `TextRole` into a text style.
`Style::underlined_text` adds an underline in the resolved text color.
`Style::GRAYSCALE` supplies optional defaults. Applications can supply another style.
The toolkit has no fixed viewport, application destinations, or feature labels.

`TextBadge` draws a label inside a rounded rectangle at a caller-supplied position.
Its `bounds` method measures the label with the role's font and the style's badge inset.
Its `render` method uses foreground fill and background text. It can also underline the label.
An empty label has zero bounds and draws nothing. The caller controls count formatting and available space.
Badges have no hit test or input behavior.

`Button` stores bounds, a label, selected state, and enabled state.
`Button::render` and `Button::hit` use that same definition.
A disabled button never passes hit testing.
Pass the hit result to `TapGesture` to require a complete tap.
The `button` function remains a convenience renderer with the default style.
Its caller supplies hit testing separately.

`NavigationCard` supplies a title, description, and direction marker inside caller-supplied bounds.
Its hit test rejects disabled cards.
The card uses relative insets and does not select a destination.

`Header` draws context, title, divider, and supplied controls.
Its caller supplies separate text regions and control bounds.
It does not clear the complete screen or process navigation.
Controls and header text clip their output to the supplied regions.
The caller must allocate sufficient space for readable labels and descriptions.
Automatic text wrapping and content pagination remain planned work.

The [application rules](../application-ui.md) select geometry, destinations, and operation restrictions.
The simulator and firmware use these components through the same application renderer.
Toolkit tests check disabled gestures and clipping with long labels and small bounds.

## Pages and shells

The [UI guide](../simulator/execution.md#ui-guide) demonstrates these contracts with a complete example application.

`Page` defines one destination's metadata, entry, exit, local touch handling, and rendering.
Its associated `Id` type belongs to the application.
`PageInfo` supplies the destination, title, context, presentation, and navigation availability.
It contains no parent or return destination.
`PageEvent` reports no change, a visible change, or a navigation request.

`PageSet` dispatches by identity to application-defined page storage.
Its `info`, `enter_page`, `leave_page`, `touch_page`, and `draw_page` methods delegate to the corresponding Page methods.
The collection can retain shared service state across related destinations.
A single Page also implements PageSet without an adapter.

`Navigator<Pages, CAPACITY>` owns one shell, the supplied page collection, and a bounded return stack.
`Navigator::launch(home, shell, pages)` enters the root exactly once before input or rendering.
It does not require a heap, global state, an executor, or trait objects.
The application selects the history capacity through the const parameter.

`Shell` supplies the shared frame and Back gesture.
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
7. Render through `Navigator::render` with the application style.

The navigator processes the Back gesture before page input.
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
The shell clears the viewport, clips standard content, and draws the header after the content.
Full-screen pages receive the complete viewport without a header.
The application and display service still control dirty state, refresh submission, and physical completion.

Overlay composition, modal input capture, automatic layout, and scrolling remain planned work.

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
