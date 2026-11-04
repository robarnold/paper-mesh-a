# Shared UI standards and visual language

[Platform overview](platform.md)

This standard defines common presentation and interaction rules for applications that use the platform toolkit.
Start with the [visual UI guide](simulator/execution.md#ui-guide) for working examples and short usage rules.
The [toolkit guide](platform/toolkit.md#pages-and-shells) defines the API and its integration requirements.
Each application selects its destinations, content, geometry, and style profile.
The demonstration's choices belong in [Application UI rules](application-ui.md).

## Principles

Keep navigation predictable and controls easy to identify.
Keep essential text readable within its assigned area.
Use words to explain unavailable data, pending work, and failures.
Keep input behavior consistent with the visible control state.
Limit display updates to meaningful visible changes.

## Structure and ownership

| Concept | Responsibility |
| --- | --- |
| Navigator | Shell launch, active destination, return history, input dispatch, and page lifecycle |
| Shell | Shared header, Back gesture, and content bounds |
| Page | One destination's content, local input, entry, exit, and presentation request |
| Control | One action or selection with shared geometry and state for drawing and hit testing |
| Style | Named text, color, and spacing roles |

The application defines page storage and destination identities through `PageSet`.
The platform navigator owns that storage and one shell after launch.
The shell does not know application features, board names, or chip details.
Page renderers use logical coordinates. The shell clips standard content to the supplied region.

Panel remains a term for a related content region. It has no separate API yet.
Overlay and modal behavior remain planned work.
A future overlay must define input capture, dismissal, and interaction with system navigation before implementation.

## Navigation and presentation

Reserve the Home key for system navigation before application button bindings run.
A Home request selects the application's home destination.
Opening a page records its caller as the return destination.
Back returns to that caller and removes the last history entry.
Pages do not declare parents. Home clears the return history.
Show Back in the upper right. Hide it on Home.
Do not draw an on-screen Home control.

A page can request full-screen presentation.
In this mode, the shell hides the header and allows content across the logical viewport.
The Home key retains its system meaning.
Give users exit instructions before entry or within the full-screen page.

A page can temporarily block navigation during an operation that must finish before departure.
Show a reason or pending state during this restriction.
Keep Back visible but disabled on a standard page.
The application defines which operations require this restriction.

The navigator leaves the previous page before entering the destination.
Cancel incomplete gestures and release page-owned work according to its workflow.
Do not interpret existing contacts as fresh input on the new page.
Entering a destination must not implicitly start a destructive or lifecycle action.

## Visual language

The initial language uses rectangular controls, a restrained grayscale palette, and explicit text.
A border identifies a destination card. A solid dark fill identifies selection or a primary action.
Use typography, spacing, and alignment to group related information.
Do not use grayscale alone to communicate status or selection meaning.

### Text roles

Choose text roles by purpose so the same screen stays readable when the style changes.
Use captions for context and body text for instructions, values, and status. Keep units with values and align comparable measurements.
Reserve underline for short emphasis. Use badges for short labels and counts.
Provide another line or page if essential text would be clipped.

### Color roles

Pair a visual state with words when the reader needs to know why an action is pending or unavailable.
Diagnostic patterns use fixed shades because their pixel values are part of the test.
For fonts, shades, and dimensions, see the [style profile](../crates/platform/src/ui/style.rs).

### Spacing roles

Use the selected profile's gaps and insets to group related information and separate adjacent targets.
Check alternate profiles on the intended display and input device.
Compare text, color, and spacing in the [visual guide](simulator/execution.md#ui-guide).

## Control and data states

Activate a touch control only after a complete press and release inside the same target.
Movement outside the target cancels the tap. Movement back inside does not restore it.
A second contact cancels the tap until all contacts lift.
A press on a disabled control must not activate after the control becomes enabled.
Use the same bounds and enabled state for drawing and hit testing.

Show a reason when an unavailable capability prevents an action.
Keep a failure visible until a new request or the workflow's specified exit.
Do not report request acceptance as physical completion.

Keep source status, observation age, and measurement values distinct.
Use `--` for missing measurements. Do not replace missing or expired data with zero.
Applications define freshness limits, update intervals, and retained reports.

## Display updates and validation

Request a frame when visible content changes.
Combine changes while a refresh is pending.
Keep a frame pending after a drawing or submission error.
Avoid animations that require repeated e-paper refreshes.

Review every component in its normal, selected, disabled, and applicable pending or failure states.
Check long text, content bounds, complete gestures, navigation restrictions, and full-screen exit paths.
Use the [visual catalog](simulator/execution.md#visual-catalog) to inspect the shared profile with the production renderer.
Keep implementation, examples, and this standard consistent when a shared rule changes.
Each guide section that demonstrates input behavior must have an interactive device preview that uses the platform renderer and input path.
Keep interactions within that preview. Do not redirect users to another guide section to try its controls.
Provide keyboard input and a reset shortcut on each focused preview except the touch-only soft keyboard.
Describe those shortcuts in the guide text.
Label separate static references explicitly.
Do not add an external state readout when the device preview already shows that state.
Use external readouts only to explain relevant behavior that the preview cannot show, such as lifecycle callbacks.
Do not place duplicate controls, keyboard control lists, or reset buttons outside the device preview.
Run repository checks in the devcontainer.
Physical display quality, touch accuracy, and optical contrast require device checks.
