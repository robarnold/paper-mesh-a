//! Retained UI elements and layout that publish into shared layers.
//!
//! # Design choices
//!
//! - Keep graphics and input independent so decorative nodes need no action.
//! - Retain explicit identities rather than infer identity from rebuilt children.
//! - Commit layout only after validation so failure preserves the previous geometry.
//!
//! See [UI](../../../../docs/platform/ui.md).

use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle};
use heapless::{String, Vec};

use super::layout::{Flow, Layout, Length};
use super::page::{Page, UiContext};
use super::{EnabledState, Style, TextRole};
use crate::composition::{self, Generation, Handler, LayerId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElementId(pub(super) usize);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Graphic<const TEXT: usize> {
    None,
    Custom(u16),
    Icon {
        bytes: &'static [u8],
        size: Size,
        ink: Gray4,
    },
    Solid(Gray4),
    Text {
        value: String<TEXT>,
        role: TextRole,
        color: Gray4,
        centered: bool,
    },
}

impl<const TEXT: usize> Graphic<TEXT> {
    pub fn text(
        value: &str,
        role: TextRole,
        color: Gray4,
        centered: bool,
    ) -> Result<Self, ElementError> {
        Ok(Self::Text {
            value: String::try_from(value).map_err(|_| ElementError::TextCapacity)?,
            role,
            color,
            centered,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Input<A> {
    pub action: A,
    pub enabled: EnabledState,
    /// Expand the hit region, then clip it to the parent's content region.
    pub outset: u32,
}

#[derive(Clone)]
pub(super) struct Node<A, const TEXT: usize> {
    pub(super) parent: Option<ElementId>,
    pub(super) layout: Layout,
    pub(super) graphic: Graphic<TEXT>,
    pub(super) input: Option<Input<A>>,
    pub(super) bounds: Rectangle,
    pub(super) clip: Rectangle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementError {
    Capacity,
    TextCapacity,
    InvalidNode,
    InvalidParent,
    InvalidGrid,
    InsufficientSpace,
}

pub type Activation<P, A> = fn(&mut P, &mut UiContext<<P as Page>::Id>, A);
pub type PanCallback<P, A> = fn(&mut P, &mut UiContext<<P as Page>::Id>, A, composition::Gesture);

/// Nodes are appended once. Identities remain valid for this collection's lifetime.
pub struct Elements<P: Page, A, const N: usize, const TEXT: usize> {
    pub(super) nodes: Vec<Node<A, TEXT>, N>,
    pub(super) viewport: Rectangle,
    pub(super) layout_dirty: bool,
    pub(super) damage: Option<Rectangle>,
    layers: [Option<LayerId>; N],
    binding_generations: [Generation; N],
    pan_callbacks: [Option<PanCallback<P, A>>; N],
    pan_data: [Option<A>; N],
    callbacks: [Option<Activation<P, A>>; N],
    fallback: Option<Activation<P, A>>,
    custom_revision: u64,
}

impl<P: Page, A: Copy + Eq, const N: usize, const TEXT: usize> Elements<P, A, N, TEXT> {
    pub const fn new(viewport: Rectangle) -> Self {
        Self {
            nodes: Vec::new(),
            viewport,
            layout_dirty: true,
            damage: Some(viewport),
            pan_callbacks: [None; N],
            pan_data: [None; N],
            binding_generations: [0; N],
            layers: [None; N],
            callbacks: [None; N],
            fallback: None,
            custom_revision: 0,
        }
    }

    pub fn add(
        &mut self,
        parent: Option<ElementId>,
        layout: Layout,
        graphic: Graphic<TEXT>,
        input: Option<Input<A>>,
    ) -> Result<ElementId, ElementError> {
        match parent {
            Some(id)
                if self
                    .nodes
                    .get(id.0)
                    .is_some_and(|n| n.layout.flow != Flow::Leaf) => {}
            None if self.nodes.is_empty() => {}
            _ => return Err(ElementError::InvalidParent),
        }
        if matches!(layout.flow, Flow::Grid { columns: 0 }) {
            return Err(ElementError::InvalidGrid);
        }
        let id = ElementId(self.nodes.len());
        self.nodes
            .push(Node {
                parent,
                layout,
                graphic,
                input,
                bounds: Rectangle::zero(),
                clip: Rectangle::zero(),
            })
            .map_err(|_| ElementError::Capacity)?;
        self.layout_dirty = true;
        Ok(id)
    }

    /// Retain a custom surface and fixed control regions within its viewport.
    /// Applications supply action values. The graphics/input stack never interprets them.
    pub fn surface(
        &mut self,
        regions: impl IntoIterator<Item = (Rectangle, A, EnabledState)>,
    ) -> Result<(), ElementError> {
        if self.nodes.is_empty() {
            self.add(
                None,
                Layout {
                    flow: Flow::Overlay,
                    ..Layout::default()
                },
                Graphic::Custom(0),
                None,
            )?;
        }
        let mut count = 1;
        for (bounds, action, enabled) in regions {
            let layout = Layout {
                offset: bounds.top_left - self.viewport.top_left,
                width: Length::Fixed(bounds.size.width),
                height: Length::Fixed(bounds.size.height),
                ..Layout::default()
            };
            let input = Some(Input {
                action,
                enabled,
                outset: 0,
            });
            if count == self.nodes.len() {
                self.add(Some(ElementId(0)), layout, Graphic::None, input)?;
            } else {
                self.set_layout(ElementId(count), layout)?;
                self.set_input(ElementId(count), input)?;
            }
            count += 1;
        }
        for index in count..self.nodes.len() {
            self.set_input(ElementId(index), None)?;
        }
        Ok(())
    }

    pub fn bounds(&self, id: ElementId) -> Result<Rectangle, ElementError> {
        Ok(self
            .nodes
            .get(id.0)
            .ok_or(ElementError::InvalidNode)?
            .bounds)
    }

    pub fn layout_dirty(&self) -> bool {
        self.layout_dirty
    }
    pub fn damage(&self) -> Option<Rectangle> {
        self.damage
    }
    pub fn input_ready(&self) -> bool {
        !self.layout_dirty
    }

    pub fn set_graphic(
        &mut self,
        id: ElementId,
        graphic: Graphic<TEXT>,
    ) -> Result<(), ElementError> {
        let node = self.nodes.get_mut(id.0).ok_or(ElementError::InvalidNode)?;
        if node.graphic == graphic {
            return Ok(());
        }
        node.graphic = graphic;
        let bounds = node.bounds;
        // Intrinsic text dimensions can affect any ancestor that measures content.
        let mut ancestor = Some(id);
        let mut intrinsic = false;
        while let Some(id) = ancestor {
            let node = &self.nodes[id.0];
            intrinsic |= matches!(node.layout.width, Length::Content)
                || matches!(node.layout.height, Length::Content);
            ancestor = node.parent;
        }
        self.damage = union(self.damage, bounds);
        if intrinsic {
            self.layout_dirty = true;
        }
        Ok(())
    }

    pub fn set_layout(&mut self, id: ElementId, layout: Layout) -> Result<(), ElementError> {
        if matches!(layout.flow, Flow::Grid { columns: 0 }) {
            return Err(ElementError::InvalidGrid);
        }
        if layout.flow == Flow::Leaf && self.nodes.iter().any(|n| n.parent == Some(id)) {
            return Err(ElementError::InvalidParent);
        }
        let node = self.nodes.get_mut(id.0).ok_or(ElementError::InvalidNode)?;
        if node.layout != layout {
            node.layout = layout;
            self.layout_dirty = true;
        }
        Ok(())
    }

    pub fn set_input(
        &mut self,
        id: ElementId,
        input: Option<Input<A>>,
    ) -> Result<(), ElementError> {
        let node = self.nodes.get_mut(id.0).ok_or(ElementError::InvalidNode)?;
        if node.input != input {
            self.binding_generations[id.0] = self.binding_generations[id.0]
                .checked_add(1)
                .expect("binding generation exhausted");
            node.input = input;
            self.damage = union(self.damage, node.bounds);
        }
        Ok(())
    }

    pub fn set_viewport(&mut self, viewport: Rectangle) {
        if self.viewport != viewport {
            self.damage = union(self.damage, self.viewport);
            self.viewport = viewport;
            self.layout_dirty = true;
        }
    }

    /// Mark custom element content for publication.
    pub fn invalidate_custom(&mut self) {
        for node in &self.nodes {
            if matches!(node.graphic, Graphic::Custom(_)) {
                self.damage = union(self.damage, node.bounds.intersection(&node.clip));
            }
        }
    }

    /// Invalidate presentation after entry, style changes, or display recovery.
    pub fn invalidate(&mut self) {
        self.damage = union(self.damage, self.viewport);
    }

    /// Remeasure after a font or other external layout dependency changes.
    pub fn invalidate_layout(&mut self) {
        self.layout_dirty = true;
        self.invalidate();
    }

    pub fn bump_handlers(&mut self) {
        for generation in &mut self.binding_generations {
            *generation = generation
                .checked_add(1)
                .expect("binding generation exhausted");
        }
    }
    pub fn did_activate(
        &mut self,
        id: ElementId,
        callback: Activation<P, A>,
    ) -> Result<(), ElementError> {
        self.nodes.get(id.0).ok_or(ElementError::InvalidNode)?;
        if self.callbacks[id.0].is_none_or(|old| !core::ptr::fn_addr_eq(old, callback)) {
            self.binding_generations[id.0] = self.binding_generations[id.0]
                .checked_add(1)
                .expect("binding generation exhausted");
        }
        self.callbacks[id.0] = Some(callback);
        Ok(())
    }
    pub fn default_callback(&mut self, callback: Activation<P, A>) {
        if self
            .fallback
            .is_none_or(|old| !core::ptr::fn_addr_eq(old, callback))
        {
            self.bump_handlers();
        }
        self.fallback = Some(callback);
    }
    pub fn root_element(&self) -> Option<ElementId> {
        (!self.nodes.is_empty()).then_some(ElementId(0))
    }
    pub fn did_pan(
        &mut self,
        id: ElementId,
        data: A,
        callback: PanCallback<P, A>,
    ) -> Result<(), ElementError> {
        self.nodes.get(id.0).ok_or(ElementError::InvalidNode)?;
        if self.pan_data[id.0] != Some(data)
            || self.pan_callbacks[id.0].is_none_or(|old| !core::ptr::fn_addr_eq(old, callback))
        {
            self.binding_generations[id.0] = self.binding_generations[id.0]
                .checked_add(1)
                .expect("binding generation exhausted");
        }
        self.pan_data[id.0] = Some(data);
        self.pan_callbacks[id.0] = Some(callback);
        Ok(())
    }
    pub fn pan_activation(&self, slot: u16) -> Option<(PanCallback<P, A>, A)> {
        let i = usize::from(slot);
        Some(((*self.pan_callbacks.get(i)?)?, (*self.pan_data.get(i)?)?))
    }
    pub fn activation(&self, slot: u16) -> Option<(Activation<P, A>, A)> {
        let index = usize::from(slot);
        let input = self.nodes.get(index)?.input?;
        if !input.enabled.is_enabled() {
            return None;
        }
        Some((self.callbacks[index].or(self.fallback)?, input.action))
    }
    pub fn set_text(&mut self, id: ElementId, text: &str) -> Result<(), ElementError> {
        let mut graphic = self
            .nodes
            .get(id.0)
            .ok_or(ElementError::InvalidNode)?
            .graphic
            .clone();
        if let Graphic::Text { value, .. } = &mut graphic {
            *value = String::try_from(text).map_err(|_| ElementError::TextCapacity)?;
            self.set_graphic(id, graphic)
        } else {
            Err(ElementError::InvalidNode)
        }
    }
    pub fn publish<const CAP: usize>(
        &mut self,
        tx: &mut composition::Transaction<'_, CAP>,
        root: LayerId,
        style: &Style,
    ) -> Result<(), composition::Error> {
        if self.damage.is_some() {
            self.custom_revision = self.custom_revision.wrapping_add(1);
        }
        tx.within(root, |tx| {
            tx.set(
                root,
                composition::Properties {
                    bounds: self.viewport,
                    ..composition::Properties::default()
                },
            )?;
            for index in 0..self.nodes.len() {
                if self.layers[index].is_none_or(|id| tx.properties(id).is_err()) {
                    self.layers[index] = Some(tx.create()?);
                }
                let n = &self.nodes[index];
                let origin = n
                    .parent
                    .map_or(self.viewport.top_left, |p| self.nodes[p.0].bounds.top_left);
                let drawing = match &n.graphic {
                    Graphic::None => composition::Drawing::None,
                    Graphic::Solid(color) => composition::Drawing::Solid(*color),
                    Graphic::Custom(slot) => composition::Drawing::Custom(*slot),
                    Graphic::Icon { bytes, size, ink } => composition::Drawing::Bitmap {
                        bytes,
                        size: *size,
                        ink: *ink,
                    },
                    Graphic::Text {
                        value,
                        role,
                        color,
                        centered,
                    } => {
                        let mut text_style = style.text(*role);
                        text_style.text_color = Some(*color);
                        composition::Drawing::Text {
                            value: heapless::String::try_from(value.as_str())
                                .map_err(|_| composition::Error::Capacity)?,
                            style: text_style,
                            centered: *centered,
                        }
                    }
                };
                let tap = n
                    .input
                    .filter(|i| {
                        i.enabled.is_enabled() && self.callbacks[index].or(self.fallback).is_some()
                    })
                    .map(|_| Handler {
                        owner: root,
                        slot: index as u16,
                        generation: self.binding_generations[index],
                    });
                tx.set(
                    self.layers[index].unwrap(),
                    composition::Properties {
                        bounds: Rectangle::new(n.bounds.top_left - origin, n.bounds.size),
                        drawing,
                        bindings: composition::Bindings {
                            tap,
                            pan: self.pan_callbacks[index].map(|_| Handler {
                                owner: root,
                                slot: index as u16,
                                generation: self.binding_generations[index],
                            }),
                        },
                        input_outset: n.input.map_or(0, |i| i.outset),
                        content_revision: if matches!(n.graphic, Graphic::Custom(_)) {
                            self.custom_revision
                        } else {
                            0
                        },
                        ..composition::Properties::default()
                    },
                )?;
            }
            for index in 0..self.nodes.len() {
                let children: Vec<LayerId, N> = self
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| n.parent == Some(ElementId(index)))
                    .map(|(i, _)| self.layers[i].unwrap())
                    .collect();
                tx.replace_children(self.layers[index].unwrap(), &children)?;
            }
            if let Some(Some(first)) = self.layers.first() {
                tx.replace_children(root, &[*first])?;
            }
            Ok(())
        })?;
        self.damage = None;
        Ok(())
    }
}

pub(super) fn union(previous: Option<Rectangle>, next: Rectangle) -> Option<Rectangle> {
    if next.size.width == 0 || next.size.height == 0 {
        return previous;
    }
    Some(match previous {
        Some(previous) => {
            let left = previous.top_left.x.min(next.top_left.x);
            let top = previous.top_left.y.min(next.top_left.y);
            let right = (previous.top_left.x + previous.size.width as i32)
                .max(next.top_left.x + next.size.width as i32);
            let bottom = (previous.top_left.y + previous.size.height as i32)
                .max(next.top_left.y + next.size.height as i32);
            Rectangle::new(
                Point::new(left, top),
                Size::new((right - left) as u32, (bottom - top) as u32),
            )
        }
        None => next,
    })
}

/// A retained button combines a visual element, label, and activation binding.
#[derive(Clone, Copy)]
pub struct Button {
    pub element: ElementId,
    pub label: ElementId,
}
impl Button {
    pub fn create<P: Page, A: Copy + Eq, const N: usize, const T: usize>(
        elements: &mut Elements<P, A, N, T>,
        parent: ElementId,
        label: &str,
        data: A,
        primary: bool,
        style: &Style,
    ) -> Result<Self, ElementError> {
        if elements.nodes.capacity() - elements.nodes.len() < 2 {
            return Err(ElementError::Capacity);
        }
        let text = Graphic::text(
            label,
            TextRole::Body,
            if primary {
                style.background
            } else {
                style.foreground
            },
            true,
        )?;
        let element = elements.add(
            Some(parent),
            Layout {
                flow: Flow::Column,
                padding: style.spacing.control_inset,
                ..Layout::default()
            },
            Graphic::Solid(if primary {
                style.foreground
            } else {
                style.control_fill
            }),
            Some(Input {
                action: data,
                enabled: EnabledState::Enabled,
                outset: 3,
            }),
        )?;
        let label = elements.add(Some(element), Layout::default(), text, None)?;
        Ok(Self { element, label })
    }
    pub fn did_activate<P: Page, A: Copy + Eq, const N: usize, const T: usize>(
        self,
        elements: &mut Elements<P, A, N, T>,
        callback: Activation<P, A>,
    ) -> Result<(), ElementError> {
        elements.did_activate(self.element, callback)
    }
    pub fn set_label<P: Page, A: Copy + Eq, const N: usize, const T: usize>(
        self,
        elements: &mut Elements<P, A, N, T>,
        value: &str,
    ) -> Result<(), ElementError> {
        elements.set_text(self.label, value)
    }
    pub fn set_enabled<P: Page, A: Copy + Eq, const N: usize, const T: usize>(
        self,
        elements: &mut Elements<P, A, N, T>,
        enabled: EnabledState,
    ) -> Result<(), ElementError> {
        let mut input = elements
            .nodes
            .get(self.element.0)
            .ok_or(ElementError::InvalidNode)?
            .input
            .ok_or(ElementError::InvalidNode)?;
        input.enabled = enabled;
        elements.set_input(self.element, Some(input))
    }
}

#[cfg(test)]
mod tests;
