//! Element measurement and placement within application-supplied bounds.
//!
//! # Design choices
//!
//! - Measure children before parents so content-sized containers include their descendants.
//! - Stage geometry until all constraints succeed so failed layout preserves the previous bounds.
//!
//! See [UI layout](../../../../docs/platform/layout.md).

use super::{
    Page, Style,
    elements::{ElementError, ElementId, Elements, Graphic, union},
};
use embedded_graphics::{prelude::*, primitives::Rectangle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Length {
    Fixed(u32),
    Content,
    Fill,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Leaf,
    Overlay,
    Row,
    Column,
    Grid { columns: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub flow: Flow,
    pub offset: Point,
    pub width: Length,
    pub height: Length,
    pub padding: u32,
    pub gap: u32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            flow: Flow::Leaf,
            offset: Point::zero(),
            width: Length::Fill,
            height: Length::Fill,
            padding: 0,
            gap: 0,
        }
    }
}

impl<P: Page, A: Copy + Eq, const N: usize, const TEXT: usize> Elements<P, A, N, TEXT> {
    /// Measure children before parents, then arrange parents before children.
    pub fn resolve(&mut self, style: &Style) -> Result<(), ElementError> {
        if !self.layout_dirty {
            return Ok(());
        }
        let mut measured = [Size::zero(); N];
        let mut geometry = [(Rectangle::zero(), Rectangle::zero()); N];
        for i in (0..self.nodes.len()).rev() {
            let node = &self.nodes[i];
            let mut natural = match &node.graphic {
                Graphic::Text { value, role, .. } => {
                    let font = style.text(*role).font;
                    Size::new(
                        value.chars().count() as u32 * font.character_size.width,
                        font.character_size.height,
                    )
                }
                Graphic::Icon { size, .. } => *size,
                _ => Size::zero(),
            };
            let children = self
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.parent == Some(ElementId(i)));
            let count = children.clone().count() as u32;
            let mut sum = Size::zero();
            let mut maximum = Size::zero();
            for (j, _) in children {
                sum = sum.saturating_add(measured[j]);
                maximum = Size::new(
                    maximum.width.max(measured[j].width),
                    maximum.height.max(measured[j].height),
                );
            }
            let gaps = node.layout.gap.saturating_mul(count.saturating_sub(1));
            let content = match node.layout.flow {
                Flow::Leaf => Size::zero(),
                Flow::Overlay => maximum,
                Flow::Row => Size::new(sum.width.saturating_add(gaps), maximum.height),
                Flow::Column => Size::new(maximum.width, sum.height.saturating_add(gaps)),
                Flow::Grid { columns } => {
                    let columns = columns as u32;
                    let rows = count.div_ceil(columns);
                    Size::new(
                        maximum.width.saturating_mul(columns).saturating_add(
                            node.layout.gap.saturating_mul(columns.saturating_sub(1)),
                        ),
                        maximum
                            .height
                            .saturating_mul(rows)
                            .saturating_add(node.layout.gap.saturating_mul(rows.saturating_sub(1))),
                    )
                }
            };
            natural = Size::new(
                natural.width.max(content.width),
                natural.height.max(content.height),
            )
            .saturating_add(Size::new(
                node.layout.padding.saturating_mul(2),
                node.layout.padding.saturating_mul(2),
            ));
            measured[i] = Size::new(
                preferred(node.layout.width, natural.width),
                preferred(node.layout.height, natural.height),
            );
        }
        if self.nodes.is_empty() {
            self.layout_dirty = false;
            return Ok(());
        }
        geometry[0] = (self.viewport, self.viewport);
        for i in 0..self.nodes.len() {
            let node = &self.nodes[i];
            let (bounds, clip) = geometry[i];
            let padding = node.layout.padding;
            if padding.saturating_mul(2) > bounds.size.width.min(bounds.size.height) {
                return Err(ElementError::InsufficientSpace);
            }
            let inner = Rectangle::new(
                bounds.top_left + Point::new(padding as i32, padding as i32),
                bounds
                    .size
                    .saturating_sub(Size::new(2 * padding, 2 * padding)),
            );
            let clip = inner.intersection(&clip);
            let children = self
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.parent == Some(ElementId(i)));
            let count = children.clone().count();
            if count == 0 {
                continue;
            }
            let row = node.layout.flow == Flow::Row;
            let mut fixed = node
                .layout
                .gap
                .saturating_mul(count.saturating_sub(1) as u32);
            let mut fills = 0u32;
            for (j, child) in children.clone() {
                let length = if row {
                    child.layout.width
                } else {
                    child.layout.height
                };
                if length == Length::Fill {
                    fills += 1;
                } else {
                    fixed = fixed.saturating_add(if row {
                        measured[j].width
                    } else {
                        measured[j].height
                    });
                }
            }
            let available = if row {
                inner.size.width
            } else {
                inner.size.height
            };
            if !matches!(node.layout.flow, Flow::Grid { .. } | Flow::Overlay) && fixed > available {
                return Err(ElementError::InsufficientSpace);
            }
            let remainder = available.saturating_sub(fixed);
            let mut cursor = 0u32;
            let mut fill_index = 0u32;
            for (index, (j, child)) in children.enumerate() {
                let cell = if node.layout.flow == Flow::Overlay {
                    Rectangle::new(inner.top_left + child.layout.offset, inner.size)
                } else if let Flow::Grid { columns } = node.layout.flow {
                    let rows = count.div_ceil(columns);
                    let xgap = node
                        .layout
                        .gap
                        .saturating_mul(columns.saturating_sub(1) as u32);
                    let ygap = node
                        .layout
                        .gap
                        .saturating_mul(rows.saturating_sub(1) as u32);
                    if xgap > inner.size.width || ygap > inner.size.height {
                        return Err(ElementError::InsufficientSpace);
                    }
                    let width = (inner.size.width - xgap) / columns as u32;
                    let height = (inner.size.height - ygap) / rows as u32;
                    Rectangle::new(
                        inner.top_left
                            + Point::new(
                                (index % columns) as i32 * (width + node.layout.gap) as i32,
                                (index / columns) as i32 * (height + node.layout.gap) as i32,
                            ),
                        Size::new(width, height),
                    )
                } else {
                    let length = if row {
                        child.layout.width
                    } else {
                        child.layout.height
                    };
                    let extent = if length == Length::Fill {
                        let extent = remainder / fills + u32::from(fill_index < remainder % fills);
                        fill_index += 1;
                        extent
                    } else if row {
                        measured[j].width
                    } else {
                        measured[j].height
                    };
                    let cell = Rectangle::new(
                        inner.top_left
                            + if row {
                                Point::new(cursor as i32, 0)
                            } else {
                                Point::new(0, cursor as i32)
                            },
                        if row {
                            Size::new(extent, inner.size.height)
                        } else {
                            Size::new(inner.size.width, extent)
                        },
                    );
                    cursor = cursor
                        .saturating_add(extent)
                        .saturating_add(node.layout.gap);
                    cell
                };
                let size = Size::new(
                    allocated(child.layout.width, measured[j].width, cell.size.width)?,
                    allocated(child.layout.height, measured[j].height, cell.size.height)?,
                );
                geometry[j] = (Rectangle::new(cell.top_left, size), clip);
            }
        }
        for (node, (bounds, clip)) in self.nodes.iter_mut().zip(geometry) {
            if node.bounds != bounds || node.clip != clip {
                self.damage = union(
                    union(self.damage, node.bounds.intersection(&node.clip)),
                    bounds.intersection(&clip),
                );
            }
            node.bounds = bounds;
            node.clip = clip;
        }
        self.layout_dirty = false;
        Ok(())
    }
}

fn preferred(length: Length, content: u32) -> u32 {
    match length {
        Length::Fixed(n) => n,
        _ => content,
    }
}
fn allocated(length: Length, measured: u32, available: u32) -> Result<u32, ElementError> {
    let size = if length == Length::Fill {
        available
    } else {
        measured
    };
    if size > available {
        Err(ElementError::InsufficientSpace)
    } else {
        Ok(size)
    }
}

#[cfg(test)]
mod tests;
