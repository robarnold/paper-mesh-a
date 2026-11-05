# Standard text field and soft keyboard

The platform toolkit provides renderers for both text entry controls.
The [visual guide](../simulator/execution.md#ui-guide) provides two focusable fields and the system keyboard.
The Keyboard diagnostic connects the controls through the following input contract.
The soft keyboard produces input events. The text field applies those events and displays the result.
The application supplies their bounds and the field capacity.
It passes each keyboard event to the field in input order.

## Keyboard output

The soft keyboard accepts key activations from UI elements. Only the touchscreen stack recognizes gestures.
A future hardware keyboard adapter can deliver text input through the focused session.
Each key activation can produce text input and change the labels.
A shifted letter produces both results. A layout key changes only the labels.
The soft keyboard emits one of two events per text key activation:

| Event | Field behavior |
| --- | --- |
| `Character(char)` | Append the character if it fits. |
| `Backspace` | Remove the last character. An empty field stays empty. |

The field owns its text and renders its current value.
Capacity is a limit in UTF-8 bytes. Reject an insertion that exceeds capacity without changing the value.
Backspace removes one complete Unicode scalar value.
The initial keyboard supplies printable ASCII characters. Comma and Space each emit one literal character per tap.

## Keyboard state and input

The keyboard starts on lowercase QWERTY. Shift toggles uppercase for the next letter.
Show the active Shift state and the case of the letters.
Emitting a letter clears Shift, even when the field is full.
Space, comma, Backspace, and canceled taps preserve Shift.

The keyboard owns its letter page and two symbol pages.
**Show common symbols** opens the common symbol page from letters or additional symbols.
**Show additional symbols** opens the additional symbol page from common symbols. That page includes underscore.
**Show letters** returns to QWERTY from either symbol page.
A page change clears Shift. Shift and page changes emit no field event.

Use the [shared tap rules](../ui.md#control-and-data-states). Holding a key produces no repeat.
During a text refresh, key meanings stay visible, so the field can accept each edit immediately.
A layout change can assign a different character to the same location.
After that change, fresh presses use the committed key bindings, even before the display shows the labels.
Binding replacement cancels an affected gesture.

## Focus ownership

The system owns the keyboard and assigns a text session to the focused field.
The application retains field values and supplies field identifiers and keyboard bounds.
Focus transfer changes the session. Old session edits are rejected rather than redirected.
The Keyboard diagnostic provides two fields to exercise this behavior.

## Example

With an empty field and capacity for two bytes:

| Tap | Output | Field | Shift |
| --- | --- | --- | --- |
| Shift | None | Empty | On |
| A | `Character('A')` | `A` | Off |
| b | `Character('b')` | `Ab` | Off |
| Backspace | `Backspace` | `A` | Off |

A third character before Backspace would exceed capacity. The field would retain `Ab`.

## Keyboard icons

The platform uses the 24-pixel ArrowUp and Erase icons from [embedded-iconoir 0.2.3](https://docs.rs/embedded-iconoir/0.2.3/embedded_iconoir/).
The [crate](https://github.com/Yandrik/embedded-iconoir/tree/c02d468425f448d7f00e2cd9fe56d05b665365e1) and its [Iconoir artwork](https://github.com/iconoir-icons/iconoir/tree/e2f9f3f94a3d85c43501894def504c3cc9d2dee3) each use the MIT license.
Distribute the [crate notice](../../crates/platform/licenses/LICENSE-embedded-iconoir.txt) and [artwork notice](../../crates/platform/licenses/LICENSE-iconoir.txt) with products that include these icons.
