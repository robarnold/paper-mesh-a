# UI layout

[UI](ui.md)

## Responsibility and status

The [layout module](../../crates/platform/src/ui/layout.rs) measures and places retained UI elements.
Applications select constraints and style. The solver writes explicit geometry for later publication to Composition.
Layout does not own graphics, gesture recognition, callback delivery, or physical display updates.
The current solver uses bounded temporary arrays without a heap.

## Constraints

`Layout` describes an element's child flow, dimensions, padding, gap, and offset.
All dimensions use logical pixels. The root receives the complete caller-supplied viewport.
Root width, height, and offset do not override that viewport.

| Length | Current meaning |
| --- | --- |
| `Fixed(n)` | Request exactly `n` pixels. Placement fails when the assigned cell cannot contain it. |
| `Content` | Use measured graphics and descendants, including padding. |
| `Fill` | Use the space assigned by the parent's flow. |

Text measurement uses the selected font's character dimensions. Icons use their declared dimensions.
Solid and custom graphics have no intrinsic size. Applications must supply suitable bounds for them.
Text does not wrap. Fixed-size content can clip, so applications must allocate space for essential text.

## Child flows

| Flow | Placement and measurement |
| --- | --- |
| `Leaf` | Accept no children. Measure only the element's graphics and padding. |
| `Overlay` | Place each child at its offset from the parent's content origin. Children share the available content size. |
| `Row` | Place children horizontally in insertion order. Sum their widths and use the maximum height for content measurement. |
| `Column` | Place children vertically in insertion order. Sum their heights and use the maximum width for content measurement. |
| `Grid { columns }` | Place children in insertion order across equal-width columns and equal-height rows. The column count must be positive. |

Padding reduces the space available for children on every edge.
Gap separates adjacent children in rows and columns, or adjacent cells in grids.
Overlay offsets apply only to children of an overlay. Overlays do not use gaps.
Overlay content measurement uses the maximum child size. Offsets do not expand that measurement.

Rows and columns reserve fixed and content-sized extents before dividing the remainder among `Fill` children.
Earlier fill children receive any leftover pixels from integer division.
The cross-axis dimension uses the child's length constraint within the available cell.
Children align with the start of their cells.

Grid content measurement uses the largest measured child width and height for each cell.
Grid placement divides the available space after gaps. Integer division can leave an unused strip at the right or bottom.
A grid does not stretch its last child across an incomplete row.
Spans, alignment choices, and weighted fill remain planned work.

## Resolution and invalidation

Parents must exist before children. Measurement visits children before parents, then placement visits parents before children.
The solver stages bounds and content clips until every constraint succeeds.
Insufficient space returns `ElementError::InsufficientSpace` and preserves the previous geometry.
Invalid parent relationships and zero-column grids are rejected when elements or constraints are added.

Layout changes, viewport changes, and content-dependent graphics changes require resolution.
Fixed-size graphics changes can require drawing without another layout pass.
Successful resolution adds old and new changed bounds to element damage.
[UI invalidation](ui.md#element-invalidation) and [Composition publication](composition.md#layers-and-atomic-updates) define the subsequent steps.

Wrapping, scrolling, and automatic element reconciliation remain outside the current solver.
The [UI requirements](ui-requirements.md) define their consumers and acceptance criteria.
