# Composition

[Platform overview](../platform.md) · [UI](ui.md)

## Responsibility

Composition owns retained layers, drawing, clipping, touchscreen gestures, and atomic publication.
It accepts explicit geometry and opaque handler tokens from clients.
It does not interpret application state or determine control layout.
[UI](ui.md) is a client of Composition.

The [implementation](../../crates/platform/src/composition.rs) uses bounded storage without a heap or an application dependency.
The system allocates one Composition store and supplies scoped roots to its clients.
Each client can modify its root and allocated descendants through a scoped transaction.
System lifecycle and storage policy belong to the [runtime](runtime.md).

## Layers and atomic updates

Each layer has parent-relative bounds, ordered children, visibility, drawing, optional input bindings, and a child-clipping flag.
Drawing can contain a solid color, text, a bitmap icon, or a custom slot.
Graphics do not imply input behavior. Decorative children cannot intercept their parent's activation.
Clipping remains hierarchical. The stack computes inherited clips during traversal.

Transactions stage changes before publication. Validation failure leaves the committed hierarchy and revision unchanged.
A successful changed batch advances the revision once. An unchanged batch does not advance it.
Replacing a parent's children atomically replaces its displayed and interactive content.
Layer handles and handler bindings use the named `Generation` type for identity versions.
Slot reuse increments the generation, so old handles cannot address replacement layers.
Detached layers retain storage and identities. Destroyed layers invalidate generation handles, including detached allocations owned by a destroyed root.

The store has a fixed layer budget selected by its caller.
The reusable layer API returns errors for capacity, ownership, geometry, and structural failures.
Clients must handle those errors before publication.

## Drawing invalidation

The layer store records these revisions separately:

| Field | Meaning |
| --- | --- |
| `committed` | Latest published content revision, including explicit invalidation. It starts at one after initialization. |
| `submitted` | Optional revision accepted for the single refresh in flight. Completion clears it. |
| `presented` | Revision acknowledged by the last completion. Zero means no revision has completed. |
| `last_submitted` | Most recently accepted revision, retained after completion. Zero means no revision has been submitted. |

`needs_display` compares `committed` with `last_submitted`. It identifies unsubmitted changes, not whether the panel is busy.
The host calls `submitted` only after successful acceptance and `completed` when that accepted refresh finishes.
The backend permits one refresh in flight. These methods do not manage a queue of submissions.
A failed submission must not advance `last_submitted`. Recovery explicitly invalidates content before retry.

Display acceptance records the submitted revision. Completion acknowledges that revision, even if newer changes exist.
Newer content therefore remains pending after an older refresh completes.
Explicit invalidation also requests a frame for custom renderers and display recovery.

The current renderer redraws the complete viewport. This removes old content after movement, detachment, or shorter text.
Revisions identify changed content. They do not identify damaged regions or select physical refresh waveforms.
The [display backend](display.md) retains buffer ownership and refresh policy.
Physical e-paper optimization remains planned work.

## Input and presentation

Touch input always uses the latest committed layers. It does not wait for submission or physical presentation.
A user can therefore activate a newly committed control before the display shows it.
This is the initial policy for delayed displays. Revisions track display work, not input permission.

Composition recognizes touchscreen taps and pans. Layers without handlers have no direct input work.
A tap requires a press and release within the same target. Input outsets remain subject to ancestor clipping.
The current `Gesture::Tap` reports only successful release. Its pending and canceled states remain internal to recognition.
Tap start and cancellation deliveries are deferred with pressed-button feedback. Callers must not infer those events from the current API.
When targets overlap, the last eligible target in drawing order wins.
Opaque graphics do not automatically capture input. Modal capture remains planned work.

An ancestor pan handler can claim movement after the threshold. Claiming a pan cancels the child tap.
Pan handlers receive start, update, end, and cancellation phases.
Detachment, binding replacement, explicit client cancellation, and additional contacts cancel affected gestures.
Clients must discard deliveries whose handlers no longer exist.

## Handler delivery

Composition stores opaque handler tokens. It does not interpret application values or call application code during traversal.
Gesture recognition produces deliveries for the owning client after traversal ends.
Clients resolve those tokens into their own behavior.
Layers without handlers do not participate as direct input targets.

## Validation and deferred work

Deterministic tests cover atomic rollback, capacity, ownership, stale handles, child replacement, inherited clips, gesture cancellation, and delayed display completion.
The Controls guide exercises ancestor pan capture through a UI client.
The navigation guide exercises atomic child replacement.

Regional damage tracking, physical e-paper optimization, and modal input capture remain deferred.
The current contract uses complete viewport redraws and explicit handler bindings.
Physical latency and touch accuracy require device validation.
