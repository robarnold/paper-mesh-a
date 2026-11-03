# Standard text field and soft keyboard

The platform toolkit will provide two controls for text entry.
The soft keyboard produces input events. The text field applies those events and displays the result.
The application supplies their bounds and the field capacity.
It passes each keyboard event to the field in input order.

## Keyboard output

The keyboard emits one of two events per text key activation:

| Event | Field behavior |
| --- | --- |
| `Character(char)` | Append the character if it fits. |
| `Backspace` | Remove the last character. An empty field stays empty. |

The field owns its text and renders its current value.
Capacity is a limit in UTF-8 bytes. Reject an insertion that exceeds capacity without changing the value.
Backspace removes one complete Unicode scalar value.
The initial keyboard supplies printable ASCII characters. Comma and Space each emit one literal character per tap.

## Keyboard state and gestures

The keyboard starts on lowercase QWERTY. Shift toggles uppercase for the next letter.
Show the active Shift state and the case of the letters.
An accepted letter clears Shift. A rejected insertion, Space, comma, or Backspace preserves it.
The caller reports insertion success to the keyboard when it delivers a letter.

The keyboard owns its letter page and two symbol pages.
**Show common symbols** opens the common symbol page from letters or additional symbols.
**Show additional symbols** opens the additional symbol page from common symbols. That page includes underscore.
**Show letters** returns to QWERTY from either symbol page.
A page change clears Shift. Shift and page changes emit no field event.

Use each visible key's bounds for hit testing.
Use the toolkit tap recognizer to activate a key once on release.
Leaving the key or adding a second contact cancels the tap. Holding a key produces no repeat.
Apply each event immediately, even during a text refresh.
After a layout change, accept a fresh press when the new labels are visible and all contacts have lifted.

## Example

With an empty field and capacity for two bytes:

| Tap | Output | Field | Shift |
| --- | --- | --- | --- |
| Shift | None | Empty | On |
| A | `Character('A')` | `A` | Off |
| b | `Character('b')` | `Ab` | Off |
| Backspace | `Backspace` | `A` | Off |

A third character before Backspace would exceed capacity. The field would retain `Ab`.
