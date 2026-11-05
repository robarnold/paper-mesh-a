# UI

[Platform overview](../platform.md) · [Composition](composition.md)

## Responsibility

UI supplies retained elements, controls, layout, styles, and application callbacks.
Applications own models, workflows, control data, and policy.
UI publishes explicit geometry, drawing, and opaque handler bindings to [Composition](composition.md).
Composition owns the layer hierarchy and touchscreen recognition.

The [element implementation](../../crates/platform/src/ui/elements.rs) uses bounded storage without a heap or an application dependency.
The system supplies a layer root to each UI client.
Pages retain elements and model state. They do not own a separate Composition store.
Custom drawing slots preserve existing diagnostic renderers within Composition's clips.

The [toolkit guide](toolkit.md#pages-and-shells) owns page lifecycle, navigation, and shell contracts.
The [runtime](runtime.md) owns storage and inactive-client reclamation policy.
The [UI requirements](ui-requirements.md) define proposed controls and their ordered backlog.
The [shared UI standard](../ui.md) remains authoritative for visual and interaction rules.

## Layout

The [UI layout contract](layout.md) defines lengths, child flows, measurement, placement, and failure behavior.
Applications select constraints. UI resolves them before publishing explicit bounds to Composition.
Element identities remain stable during updates. Element construction is currently append-only.

## Element invalidation

UI tracks layout invalidation separately from drawing damage.
A graphics change marks the affected element bounds. Identical graphics do not add damage.
A content-sized element can require measurement after a text change.
Layout resolution records old and new bounds when geometry changes.

Element damage is local bookkeeping. Publication clears it after updating the staged layers.
The current page host can also invalidate the complete page after an application change.
These mechanisms do not provide end-to-end regional redraw guarantees.
Composition's [revision contract](composition.md#drawing-invalidation) determines pending display work.
Pressed-button feedback and its damage-region test remain deferred.

## Controls and callbacks

UI resolves a Composition delivery to a callback after traversal ends.
Callbacks receive mutable page state, a UI context, and typed control data.
They can update elements and request navigation without borrowing the layer store during hit testing.

Callbacks use function pointers. Noncapturing Rust closures provide the application syntax without heap allocation or self-referential state.
Captured values belong in page state or explicit per-control data. General capturing closures remain unsupported.
The retained `Button` helper combines a solid element, a text child, and a callback binding.
Text and icon graphics use the same retained element API.
Input bindings and custom surfaces use `EnabledState::Enabled` or `EnabledState::Disabled`.
`EnabledState::from(condition)` is the canonical conversion from application predicates.

Handler names follow the [event handler naming rule](../../DOCUMENTATION.md#event-handler-names).
`did_activate` runs after a completed tap. `did_pan` receives recognized pan phases.
The current tap contract has no start or cancellation callback.

Cancellation callbacks release control state. They cannot initiate another navigation transition.
The [navigation contract](toolkit.md#pages-and-shells) defines transition cancellation and held-contact suppression.

## Focus and keyboard

The system owns focus and the soft keyboard. Both use UI rather than introducing another input runtime.
The keyboard builds on UI elements and receives ordinary key activations.
It has no gesture recognizer. Its output contains text edits, not touch gestures.
The [text input contract](text-input.md) owns focus-session rules, keyboard state, and field editing behavior.

## Examples and validation

The guide's calculator uses production elements, button callbacks, layout, and Composition.
The Controls example demonstrates ancestor pan capture and child tap cancellation.
The Text input guide demonstrates focus transfer between two fields using the system keyboard.

Tests cover layout failure, callback updates, focus sessions, unchanged publication, and retained application state.
Simulator scenarios cover calculator edits before presentation, Back, Home, and idle periods without display work.
Each shared control requires an interactive guide example under the [shared validation rules](../ui.md#display-updates-and-validation).
