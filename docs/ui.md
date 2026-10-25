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

`TextRole` names a purpose, even when several roles currently share a font.
Applications select roles instead of individual fonts and shades in each renderer.

| Role | Use | Default profile |
| --- | --- | --- |
| Title | Page and section names | `FONT_10X20`, 10 × 20 px, foreground |
| Body | Instructions and measurement values | `FONT_10X20`, 10 × 20 px, foreground |
| Caption | Short context and group labels | `FONT_6X10`, 6 × 10 px, foreground |
| Secondary | Supporting information | `FONT_10X20`, 10 × 20 px, secondary text |

The default profile uses the built-in ASCII monospace bitmap fonts from `embedded-graphics`.
Sizes give character-cell width × height in logical device pixels.
Stroke weight is fixed in each bitmap. The profile has no bold variant or numeric weight setting.
Title and Body share the same font, size, and weight. Secondary uses a lighter color.
Selected controls invert colors without changing font weight.

Each role has a normal variant through `Style::text(role)` and an underlined variant through `Style::underlined_text(role)`.
Underline is a decoration, not a separate text role or font weight.
It preserves the role's font, size, and color. The current fonts have no italic variant.
Use underline for short emphasis. An underline alone does not identify an action or explain status.

`TextBadge` supplies bubbled text for short labels and notification counts.
It measures the label and adds a rounded rectangle with padding on all sides.
The bubble uses the foreground color. Its text uses the background color for contrast and retains the selected role's font and size.
The default profile gives white text on a black bubble, with an 8-pixel corner radius.
Underline can also apply inside a bubble.

Badges display information. Use a separate control for actions.
The application supplies the label and decides when to hide or cap a count.

The header context and short group labels use `Caption`.
In the visual catalog, `PLATFORM UI` and `TEXT ROLES` show this role.
Controls select their own label style from the shared profile.
Use `Body` for status messages. Explain pending work, unavailable data, and failures with explicit words.

Use short titles. Keep units with values and align comparable values.
Use additional lines or pages when text does not fit.
Clipping protects adjacent regions. It does not make truncated essential text acceptable.
Automatic wrapping and pagination remain planned work.

### Color roles

The default `Style::GRAYSCALE` profile uses 16 shades, from 0 for black to 15 for white.

| Role | Shade | Use |
| --- | --- | --- |
| Foreground | 0 | Essential text, outlines, and selected control fill |
| Background | 15 | Page background and selected control text |
| Secondary text | 6 | Supporting text |
| Divider | 9 | Group boundaries and data outlines |
| Control fill | 14 | Unselected and disabled control surfaces |
| Disabled text | 8 | Controls that cannot accept input |
| Data fill | 5 | Filled measurement bars |
| Alternating row | 12 | Separation within dense data lists |

Selected controls use inverse text. Disabled controls do not use the selected fill.
A pending label explains an operation in progress. Disabled appearance alone does not explain the cause.
Diagnostic image values are test data and do not use theme colors.

### Spacing roles

| Role | Default pixels | Use |
| --- | --- | --- |
| Badge inset | 8 horizontal, 4 vertical | Padding on each side of a badge label |
| Control inset | 16 | Leading space inside a button |
| Card inset | 20 | Space inside a destination card |
| Control gap | 12 | Separation between adjacent controls |
| Section gap | 28 | Separation between content groups |
| Minimum control height | 52 | Minimum height for a touch target |

These values form one initial profile. They do not define a universal viewport or fixed page layout.
The application supplies geometry that follows its chosen profile.
Validate alternate profiles on their intended display and input device.

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
Each visual guide section must have an interactive device preview that uses the platform renderer and input path.
Keep interactions within that preview. Do not redirect users to another guide section to try its controls.
Provide equivalent keyboard controls and an independent reset action for each example.
Label separate static references explicitly.
Run repository checks in the devcontainer.
Physical display quality, touch accuracy, and optical contrast require device checks.
