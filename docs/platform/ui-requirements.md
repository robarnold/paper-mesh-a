# UI requirements

[Toolkit guide](toolkit.md)

## Purpose and status

These proposed contracts support the [aspirational application flows](../application-flows.md).
This document owns missing UI capabilities, their consumers, and acceptance criteria.
It does not redefine current behavior in [UI](ui.md), [Composition](composition.md), or the [shared UI standard](../ui.md).

UI currently supplies retained elements, bounded layout, callback bindings, and system-managed text focus.
The [text input contract](text-input.md) supports append and backspace, without general cursor editing or form traversal.
The sections below describe proposed behavior unless explicitly identified as current.

## Requirement boundaries

Applications retain models, identities, validation, service requests, collection limits, and overflow policy.
UI components retain control state and compute layout within application-supplied bounds.
Composition owns layers, clipping, touch recognition, and atomic updates.
UI controls must use that contract instead of introducing separate gesture processing.

New components must remain bounded and compatible with `no_std`.
They must not require heap allocation, global state, an executor, or an application dependency.
Capacity exhaustion must produce an explicit result without silently deleting existing content.

## Layout, text, and collections

Rows, columns, insets, and gaps compose within caller-supplied bounds.
Measurement must account for the selected text roles and wrapped content before input bounds are assigned.
Insufficient space must be observable to the caller.
Essential text requires wrapping, scrolling, or a detail destination rather than silent clipping.
Fixed application layouts and backend dimensions remain outside these helpers.

A reusable list row combines a label, optional secondary text, value, status, and caller-selected action.
Selection and enabled state use words or marks as well as shade.
Stable item identities preserve selection across reordered service results.
The application decides when new results replace the visible list.

A viewport uses one offset and clipping region for rendering and hit testing.
Initial scrolling uses explicit Previous and Next actions with a visible position indicator.
Each step advances a bounded visible portion and works with variable-height rows.
Long individual entries must remain readable across steps.
Changing the viewport offset cancels any armed row tap.
Collection shrinkage clamps the offset and clears selection only when the selected identity disappears.
Continuous animation and drag scrolling are not required for the first implementation.

## Forms, focus, and selection

A form composes labels, fields, help, validation messages, and action controls.
One field receives text input at a time. Focus must remain visible and identify the keyboard's target.
Keyboard placement reduces the available content area without covering the focused field or its validation message.
Focus changes must reject old-session edits rather than redirect them to another field.
Layout changes publish through Composition. Input uses committed bindings under its [input contract](composition.md#input-and-presentation).

Text editing requires insertion and deletion at a visible cursor, with application-selected capacity.
Multiline fields support message drafts. Password fields conceal content and expose an explicit reveal action.
Numeric fields expose suitable keys while applications validate units, ranges, and decimal policy.
Text fields report rejected input so callers can explain capacity or validation limits.
The application decides when edits commit, cancel, or remain drafts.
Credential storage and clearing policy belong to the connection workflow, not a general form.

Toggles and single-choice controls emit requested values.
They must support a pending state separate from the confirmed value.
Numeric adjustment controls emit bounded edit actions under caller-supplied limits.
These controls must not infer service completion or durable storage from user activation.

## Status, dialogs, and overlays

Status regions compose explanatory text with optional progress, retry, cancel, or connection actions.
They support empty, unavailable, pending, success, and failure presentations without application-specific error enums.
The application supplies wording, action availability, dismissal policy, and progress evidence.
Indeterminate work uses stable text rather than an animation that requires repeated refreshes.

An overlay is a presentation layer above page content.
A modal overlay captures all ordinary page input and makes covered controls inactive.
The initial contract permits one modal at a time. Another request must return an explicit busy result.
The caller decides whether to defer or reject that request.
Touch outside a dialog does not confirm or dismiss it.
Opening a dialog cancels the underlying gesture and records the previous focus.

Back cancels a dismissible dialog and consumes that navigation request.
Home cancels the dialog and then follows the existing Home navigation contract.
Neither action implies approval or physical cancellation of completed service work.
A dialog cannot independently disable system navigation.
An operation that requires blocked navigation must use the existing page restriction and show its reason.
After dismissal, focus returns to its previous control if that control still exists and remains enabled.
Otherwise, the page chooses an eligible focus target.
Dismissal never forwards the initiating contact to the uncovered page.

These proposed modal rules require implementation and guide coverage before applications can depend on them.

## Ordered implementation backlog

Current [UI](ui.md) behavior covers bounded rows, columns, grids, independent input regions, and element damage bookkeeping.
The calculator is the first application composition. Wrapping, lists, scrolling, and the remaining items below are still incomplete.

The order below follows shared use across applications.
Each row defines a separate acceptance boundary. No application service implementation is implied.

| Priority and item | Consumers | Existing basis | Missing contract | Acceptance scenario |
| --- | --- | --- | --- | --- |
| 1. Layout and wrapped text | All six flows | Measured rows, columns, grids, insets, overflow errors | Wrapping and wider layout coverage | Long labels fit or report insufficient space. Draw and input bounds agree. |
| 1. List rows | Conversations, waypoints, Wi-Fi, Bluetooth, settings | Cards, buttons, badges | Compound rows, stable identity, selection | Reorder equal-name devices. Selection follows identity rather than index. |
| 1. Bounded scrolling | Message history and all collections | Logical coordinates and tap cancellation | Viewport offset, clipping, explicit steps | Scroll while a row is armed. No row activates after the offset changes. |
| 2. Forms and focus | Message drafts, waypoint editors, credentials, settings | Text fields, system keyboard, focus sessions | Form traversal, field visibility, validation placement | Switch fields with the keyboard visible. Each edit reaches only its selected field. |
| 2. Text editing | Messages and named records | Append and backspace events | Cursor insertion, multiline rendering, rejected-input result, concealment | Insert within text at capacity. Rejection preserves text and explains the limit. |
| 2. Selection and numeric controls | Radio state, settings, coordinates, calculator | Selected and disabled buttons | Requested versus confirmed values, numeric actions, keypad composition | Request a toggle and fail completion. The confirmed value does not change. |
| 3. Status presentations | Every service-dependent flow and calculator errors | Badges, text, buttons | Common state composition with caller-owned actions | Replace pending with failure. Retry appears without discarding the user's input. |
| 3. Dialog and overlay routing | Forget, delete, defaults, Bluetooth pairing | Navigator and page restrictions | Capture, dismissal, focus restoration, modal admission | Open a dialog during a tap, dismiss it, then release. No covered control activates. |
| 4. Application compositions | All six flows | Earlier backlog items | Application controllers and retained models | Complete each application scenario with deterministic service fixtures. |

Application compositions must keep conversation models, waypoint calculations, connection policy, and arithmetic in the application crate.
Shared components must not include fixed destinations or application service choices.

## Validation requirements

Each implemented component needs deterministic interaction tests and an interactive example in the visual guide.
Examples must use the production renderer and input path under the [shared validation rules](../ui.md#display-updates-and-validation).
Static references alone do not establish input behavior.

Layout tests must cover narrow bounds, long text, different text roles, and insufficient space.
Collection tests must cover empty content, full capacity, reordering, removal, and a single oversized entry.
Input tests must cover disabled controls, multiple contacts, cancellation, keyboard changes, and modal dismissal.
Navigation tests must cover Back, Home, retained drafts, and page restrictions during pending work.
Rendering checks must cover normal, selected, disabled, pending, and failure states.
Display submission failure must preserve the requested redraw under the existing display contract.

The [application scenarios](../application-flows.md#deterministic-acceptance-scenarios) define the later integration checks.
Simulator results cannot establish physical readability, touch accuracy, or hardware service support.
