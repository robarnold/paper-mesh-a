//! Atomic retained graphics and touchscreen input, independent of UI elements.
//!
//! # Design choices
//!
//! - Stage edits in bounded storage so failed publication cannot expose partial geometry.
//! - Keep handlers opaque so this layer has no application or control policy.
//! - Derive inherited clipping during traversal so parents can change it atomically.
//!
//! See [Composition](../../../docs/platform/composition.md).

use crate::TouchEvent;
use embedded_graphics::{
    draw_target::Clipped,
    mono_font::MonoTextStyle,
    pixelcolor::Gray4,
    prelude::*,
    primitives::Rectangle,
    text::{Baseline, Text},
};
use heapless::{String, Vec};

/// Identity version for layer slots and handler bindings.
pub type Generation = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerId {
    index: core::num::NonZeroU16,
    generation: Generation,
}
impl LayerId {
    // Reserve zero so optional links occupy the same space as a live handle.
    fn new(index: usize, generation: Generation) -> Self {
        Self {
            index: core::num::NonZeroU16::new((index + 1) as u16).expect("checked layer capacity"),
            generation,
        }
    }
    fn index(self) -> usize {
        usize::from(self.index.get()) - 1
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handler {
    pub owner: LayerId,
    pub slot: u16,
    pub generation: Generation,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bindings {
    pub tap: Option<Handler>,
    pub pan: Option<Handler>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanPhase {
    Start,
    Update,
    End,
    Cancel,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Tap,
    Pan {
        phase: PanPhase,
        position: Point,
        displacement: Point,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Delivery {
    pub handler: Handler,
    pub gesture: Gesture,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Drawing {
    None,
    Solid(Gray4),
    Text {
        value: String<64>,
        style: MonoTextStyle<'static, Gray4>,
        centered: bool,
    },
    Bitmap {
        bytes: &'static [u8],
        size: Size,
        ink: Gray4,
    },
    Custom(u16),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Properties {
    pub bounds: Rectangle,
    pub clip_children: bool,
    pub visible: bool,
    pub drawing: Drawing,
    pub bindings: Bindings,
    pub input_outset: u32,
    pub content_revision: u64,
}
impl Default for Properties {
    fn default() -> Self {
        Self {
            bounds: Rectangle::zero(),
            clip_children: true,
            visible: true,
            drawing: Drawing::None,
            bindings: Bindings::default(),
            input_outset: 0,
            content_revision: 0,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
struct Node {
    owner: LayerId,
    parent: Option<LayerId>,
    first: Option<LayerId>,
    last: Option<LayerId>,
    previous: Option<LayerId>,
    next: Option<LayerId>,
    interest: bool,
    properties: Properties,
}
#[derive(Clone, Debug, PartialEq)]
struct Slot {
    generation: Generation,
    node: Option<Node>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Capacity,
    StaleHandle,
    Ownership,
    Cycle,
    InvalidChildren,
    InvalidGeometry,
    Poisoned,
}
#[derive(Clone, Copy)]
struct Contact {
    id: u8,
    start: Point,
    position: Point,
    tap: Option<(LayerId, Handler)>,
    pan: Option<(LayerId, Handler)>,
    panning: bool,
}

/// The host allocates this store in static storage on constrained targets.
pub struct Layers<const N: usize> {
    live: Vec<Slot, N>,
    staging: Vec<Slot, N>,
    root: Option<LayerId>,
    committed: u64,
    submitted: Option<u64>,
    presented: u64,
    last_submitted: u64,
    contact: Option<Contact>,
    contacts: u16,
    cancellations: Vec<Delivery, 2>,
    pub pan_threshold: u32,
}
impl<const N: usize> Default for Layers<N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const N: usize> Layers<N> {
    pub const fn new() -> Self {
        Self {
            live: Vec::new(),
            staging: Vec::new(),
            root: None,
            committed: 0,
            submitted: None,
            presented: 0,
            last_submitted: 0,
            contact: None,
            contacts: 0,
            cancellations: Vec::new(),
            pan_threshold: 8,
        }
    }
    pub fn initialize(&mut self, viewport: Rectangle) -> Result<LayerId, Error> {
        if let Some(root) = self.root {
            return Ok(root);
        }
        let root = LayerId::new(0, 0);
        self.live
            .push(Slot {
                generation: 0,
                node: Some(Node {
                    owner: root,
                    parent: None,
                    first: None,
                    last: None,
                    previous: None,
                    next: None,
                    interest: false,
                    properties: Properties {
                        bounds: viewport,
                        ..Properties::default()
                    },
                }),
            })
            .map_err(|_| Error::Capacity)?;
        self.root = Some(root);
        self.committed = 1;
        Ok(root)
    }
    pub fn root(&self) -> LayerId {
        self.root.expect("layers initialized")
    }
    pub fn revision(&self) -> u64 {
        self.committed
    }
    pub fn presented_revision(&self) -> u64 {
        self.presented
    }
    pub fn needs_display(&self) -> bool {
        self.committed != self.last_submitted
    }
    pub fn submitted(&mut self) {
        self.submitted = Some(self.committed);
        self.last_submitted = self.committed;
    }
    pub fn completed(&mut self) {
        if let Some(r) = self.submitted.take() {
            self.presented = r;
        }
    }
    pub fn invalidate(&mut self) {
        self.committed = self.committed.checked_add(1).expect("revision exhausted");
    }
    pub fn begin(&mut self, root: LayerId) -> Result<Transaction<'_, N>, Error> {
        if node(&self.live, root)?.owner != root {
            return Err(Error::Ownership);
        }
        self.staging.clear();
        for slot in &self.live {
            self.staging
                .push(slot.clone())
                .map_err(|_| Error::Capacity)?;
        }
        Ok(Transaction {
            store: self,
            scope: root,
            failed: false,
        })
    }
    pub fn properties(&self, id: LayerId) -> Result<&Properties, Error> {
        Ok(&node(&self.live, id)?.properties)
    }
    fn connected(&self, mut id: LayerId) -> bool {
        for _ in 0..N {
            let Ok(n) = node(&self.live, id) else {
                return false;
            };
            if !n.properties.visible {
                return false;
            }
            if Some(id) == self.root {
                return true;
            }
            let Some(parent) = n.parent else {
                return false;
            };
            id = parent;
        }
        false
    }
    fn origin(&self, mut id: LayerId) -> Point {
        let mut x = 0_i64;
        let mut y = 0_i64;
        for _ in 0..N {
            let n = node(&self.live, id).unwrap();
            x += i64::from(n.properties.bounds.top_left.x);
            y += i64::from(n.properties.bounds.top_left.y);
            if let Some(p) = n.parent {
                id = p;
            } else {
                break;
            }
        }
        Point::new(x as i32, y as i32)
    }
    pub fn geometry(&self, id: LayerId) -> Result<(Rectangle, Rectangle), Error> {
        let n = node(&self.live, id)?;
        let bounds = Rectangle::new(self.origin(id), n.properties.bounds.size);
        let mut clip = node(&self.live, self.root())?.properties.bounds;
        let mut parent = n.parent;
        while let Some(id) = parent {
            let p = node(&self.live, id)?;
            if p.properties.clip_children {
                clip =
                    clip.intersection(&Rectangle::new(self.origin(id), p.properties.bounds.size));
            }
            parent = p.parent;
        }
        Ok((bounds, clip))
    }
    fn next(&self, id: LayerId, input: bool) -> Option<LayerId> {
        let n = node(&self.live, id).ok()?;
        if n.properties.visible && (!input || n.interest) && n.first.is_some() {
            return n.first;
        }
        let mut current = id;
        loop {
            let n = node(&self.live, current).ok()?;
            if n.next.is_some() {
                return n.next;
            }
            current = n.parent?;
        }
    }
    fn hit(&self, position: Point) -> Option<LayerId> {
        let mut current = self.root;
        let mut hit = None;
        while let Some(id) = current {
            let n = node(&self.live, id).unwrap();
            if n.properties.visible
                && (n.properties.bindings.tap.is_some() || n.properties.bindings.pan.is_some())
            {
                let (bounds, clip) = self.geometry(id).unwrap();
                let outset = n.properties.input_outset.min(i32::MAX as u32 / 2) as i32;
                if bounds.offset(outset).intersection(&clip).contains(position) {
                    hit = Some(id);
                }
            }
            current = self.next(id, true);
        }
        hit
    }
    fn cancel(&mut self) {
        if let Some(c) = self.contact.take()
            && c.panning
            && let Some((_, handler)) = c.pan
        {
            let _ = self.cancellations.push(Delivery {
                handler,
                gesture: Gesture::Pan {
                    phase: PanPhase::Cancel,
                    position: c.position,
                    displacement: displacement(c.position, c.start),
                },
            });
        }
    }
    pub fn contains_handler(&self, handler: Handler) -> bool {
        self.live.iter().filter_map(|s| s.node.as_ref()).any(|n| {
            n.properties.bindings.tap == Some(handler) || n.properties.bindings.pan == Some(handler)
        })
    }
    pub fn take_cancellation(&mut self) -> Option<Delivery> {
        self.cancellations.pop()
    }
    /// Recognition uses committed layers immediately, regardless of display progress.
    pub fn touch(&mut self, event: TouchEvent) -> Option<Delivery> {
        let (contact, release) = match event {
            TouchEvent::Cancel => {
                self.cancel();
                self.contacts = 0;
                return self.take_cancellation();
            }
            TouchEvent::Touch(c) => (c, false),
            TouchEvent::Release(c) => (c, true),
        };
        let Some(bit) = 1u16.checked_shl(contact.id.into()) else {
            self.cancel();
            return self.take_cancellation();
        };
        if !release && self.contacts == 0 {
            let hit = self.hit(contact.position);
            let tap = hit.and_then(|id| {
                node(&self.live, id)
                    .unwrap()
                    .properties
                    .bindings
                    .tap
                    .map(|h| (id, h))
            });
            let mut ancestor = hit;
            let mut pan = None;
            while let Some(id) = ancestor {
                let n = node(&self.live, id).unwrap();
                if let Some(h) = n.properties.bindings.pan {
                    pan = Some((id, h));
                    break;
                }
                ancestor = n.parent;
            }
            self.contact = Some(Contact {
                id: contact.id,
                start: contact.position,
                position: contact.position,
                tap,
                pan,
                panning: false,
            });
        }
        if release {
            self.contacts &= !bit;
        } else {
            self.contacts |= bit;
        }
        if self.contacts.count_ones() > 1 {
            self.cancel();
            return self.take_cancellation();
        }
        let mut c = self.contact?;
        if c.id != contact.id {
            self.cancel();
            return self.take_cancellation();
        }
        c.position = contact.position;
        if !c.panning {
            if c.tap
                .is_some_and(|(id, _)| self.hit(c.position) != Some(id))
            {
                c.tap = None;
            }
            let delta = displacement(c.position, c.start);
            let distance = i128::from(delta.x).pow(2) + i128::from(delta.y).pow(2);
            if !release
                && distance >= i128::from(self.pan_threshold).pow(2)
                && let Some((_, handler)) = c.pan
            {
                c.panning = true;
                c.tap = None;
                self.contact = Some(c);
                return Some(Delivery {
                    handler,
                    gesture: Gesture::Pan {
                        phase: PanPhase::Start,
                        position: c.position,
                        displacement: delta,
                    },
                });
            }
        }
        self.contact = if release { None } else { Some(c) };
        if c.panning {
            let (_, handler) = c.pan?;
            Some(Delivery {
                handler,
                gesture: Gesture::Pan {
                    phase: if release {
                        PanPhase::End
                    } else {
                        PanPhase::Update
                    },
                    position: c.position,
                    displacement: displacement(c.position, c.start),
                },
            })
        } else if release && self.contacts == 0 {
            let (_, handler) = c.tap?;
            Some(Delivery {
                handler,
                gesture: Gesture::Tap,
            })
        } else {
            None
        }
    }
    pub fn render<D, F>(&self, target: &mut D, mut custom: F) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
        F: FnMut(LayerId, u16, &mut Clipped<'_, D>) -> Result<(), D::Error>,
    {
        let mut current = self.root;
        while let Some(id) = current {
            let n = node(&self.live, id).unwrap();
            if n.properties.visible {
                let (bounds, clip) = self.geometry(id).unwrap();
                let mut out = target.clipped(&bounds.intersection(&clip));
                match &n.properties.drawing {
                    Drawing::None => {}
                    Drawing::Solid(color) => out.clear(*color)?,
                    Drawing::Custom(slot) => custom(n.owner, *slot, &mut out)?,
                    Drawing::Text {
                        value,
                        style,
                        centered,
                    } => {
                        let width = value.chars().count() as u32 * style.font.character_size.width;
                        let x = if *centered {
                            bounds.size.width.saturating_sub(width) / 2
                        } else {
                            0
                        };
                        let y = bounds
                            .size
                            .height
                            .saturating_sub(style.font.character_size.height)
                            / 2;
                        Text::with_baseline(
                            value,
                            bounds.top_left + Point::new(x as i32, y as i32),
                            *style,
                            Baseline::Top,
                        )
                        .draw(&mut out)?;
                    }
                    Drawing::Bitmap { bytes, size, ink } => {
                        out.draw_iter((0..size.height).flat_map(|y| {
                            (0..size.width).filter_map(move |x| {
                                let i = (y * size.width + x) as usize;
                                bytes
                                    .get(i / 8)
                                    .filter(|b| **b & (0x80 >> (i % 8)) != 0)
                                    .map(|_| {
                                        Pixel(
                                            bounds.top_left + Point::new(x as i32, y as i32),
                                            *ink,
                                        )
                                    })
                            })
                        }))?;
                    }
                }
            }
            current = self.next(id, false);
        }
        Ok(())
    }
}
fn node<const N: usize>(slots: &Vec<Slot, N>, id: LayerId) -> Result<&Node, Error> {
    let s = slots.get(id.index()).ok_or(Error::StaleHandle)?;
    if s.generation != id.generation {
        return Err(Error::StaleHandle);
    }
    s.node.as_ref().ok_or(Error::StaleHandle)
}
fn node_mut<const N: usize>(slots: &mut Vec<Slot, N>, id: LayerId) -> Result<&mut Node, Error> {
    let s = slots.get_mut(id.index()).ok_or(Error::StaleHandle)?;
    if s.generation != id.generation {
        return Err(Error::StaleHandle);
    }
    s.node.as_mut().ok_or(Error::StaleHandle)
}

pub struct Transaction<'a, const N: usize> {
    store: &'a mut Layers<N>,
    scope: LayerId,
    failed: bool,
}
impl<const N: usize> Transaction<'_, N> {
    fn check(&self, id: LayerId) -> Result<(), Error> {
        let n = node(&self.store.staging, id)?;
        if self.scope == self.store.root() || id == self.scope || n.owner == self.scope {
            Ok(())
        } else {
            Err(Error::Ownership)
        }
    }
    fn result<T>(&mut self, r: Result<T, Error>) -> Result<T, Error> {
        if r.is_err() {
            self.failed = true;
        }
        r
    }
    pub fn within<T>(
        &mut self,
        root: LayerId,
        f: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let check = self.check(root).and_then(|()| {
            if node(&self.store.staging, root)?.owner != root {
                return Err(Error::Ownership);
            }
            Ok(())
        });
        self.result(check)?;
        let previous = self.scope;
        self.scope = root;
        let result = f(self);
        self.scope = previous;
        self.result(result)
    }
    pub fn create(&mut self) -> Result<LayerId, Error> {
        let r = self.allocate(false);
        self.result(r)
    }
    pub fn create_root(&mut self) -> Result<LayerId, Error> {
        let r = if self.scope != self.store.root() {
            Err(Error::Ownership)
        } else {
            self.allocate(true)
        };
        self.result(r)
    }
    fn allocate(&mut self, root: bool) -> Result<LayerId, Error> {
        let i = self
            .store
            .staging
            .iter()
            .position(|s| s.node.is_none())
            .unwrap_or(self.store.staging.len());
        if i >= u16::MAX as usize {
            return Err(Error::Capacity);
        }
        if i == self.store.staging.len() {
            self.store
                .staging
                .push(Slot {
                    generation: 0,
                    node: None,
                })
                .map_err(|_| Error::Capacity)?;
        }
        let slot = &mut self.store.staging[i];
        let id = LayerId::new(i, slot.generation);
        slot.node = Some(Node {
            owner: if root { id } else { self.scope },
            parent: None,
            first: None,
            last: None,
            previous: None,
            next: None,
            interest: false,
            properties: Properties::default(),
        });
        Ok(id)
    }
    pub fn set(&mut self, id: LayerId, properties: Properties) -> Result<(), Error> {
        let r = self.check(id).and_then(|()| {
            let n = node_mut(&mut self.store.staging, id)?;
            if [properties.bindings.tap, properties.bindings.pan]
                .into_iter()
                .flatten()
                .any(|h| h.owner != n.owner)
            {
                return Err(Error::Ownership);
            }
            n.properties = properties;
            Ok(())
        });
        self.result(r)
    }
    pub fn properties(&self, id: LayerId) -> Result<&Properties, Error> {
        self.check(id)?;
        Ok(&node(&self.store.staging, id)?.properties)
    }
    fn unlink(&mut self, id: LayerId) -> Result<(), Error> {
        let n = node(&self.store.staging, id)?.clone();
        if let Some(parent) = n.parent {
            if let Some(prev) = n.previous {
                node_mut(&mut self.store.staging, prev)?.next = n.next;
            } else {
                node_mut(&mut self.store.staging, parent)?.first = n.next;
            }
            if let Some(next) = n.next {
                node_mut(&mut self.store.staging, next)?.previous = n.previous;
            } else {
                node_mut(&mut self.store.staging, parent)?.last = n.previous;
            }
        }
        let n = node_mut(&mut self.store.staging, id)?;
        n.parent = None;
        n.previous = None;
        n.next = None;
        Ok(())
    }
    pub fn detach(&mut self, id: LayerId) -> Result<(), Error> {
        let r = self.check(id).and_then(|()| {
            if id == self.scope && self.scope != self.store.root() {
                return Err(Error::Ownership);
            }
            self.unlink(id)
        });
        self.result(r)
    }
    pub fn replace_children(&mut self, parent: LayerId, children: &[LayerId]) -> Result<(), Error> {
        let r = self.replace(parent, children);
        self.result(r)
    }
    fn replace(&mut self, parent: LayerId, children: &[LayerId]) -> Result<(), Error> {
        self.check(parent)?;
        for (i, &id) in children.iter().enumerate() {
            self.check(id)?;
            if children[..i].contains(&id) {
                return Err(Error::InvalidChildren);
            }
            let mut p = Some(parent);
            while let Some(ancestor) = p {
                if ancestor == id {
                    return Err(Error::Cycle);
                }
                p = node(&self.store.staging, ancestor)?.parent;
            }
        }
        while let Some(child) = node(&self.store.staging, parent)?.first {
            self.unlink(child)?;
        }
        for &child in children {
            self.unlink(child)?;
            let previous = node(&self.store.staging, parent)?.last;
            let c = node_mut(&mut self.store.staging, child)?;
            c.parent = Some(parent);
            c.previous = previous;
            if let Some(prev) = previous {
                node_mut(&mut self.store.staging, prev)?.next = Some(child);
            } else {
                node_mut(&mut self.store.staging, parent)?.first = Some(child);
            }
            node_mut(&mut self.store.staging, parent)?.last = Some(child);
        }
        Ok(())
    }
    pub fn destroy(&mut self, id: LayerId) -> Result<(), Error> {
        let r = self.remove(id);
        self.result(r)
    }
    fn remove(&mut self, id: LayerId) -> Result<(), Error> {
        self.check(id)?;
        if id == self.store.root() {
            return Err(Error::Ownership);
        }
        self.unlink(id)?;
        // Remove descendants before parents; destroying an owner also revokes detached allocations.
        let mut doomed = Vec::<LayerId, N>::new();
        for (index, s) in self.store.staging.iter().enumerate() {
            if let Some(n) = &s.node {
                let candidate = LayerId::new(index, s.generation);
                let mut p = Some(candidate);
                let mut remove = n.owner == id;
                while let Some(a) = p {
                    if a == id {
                        remove = true;
                        break;
                    }
                    p = node(&self.store.staging, a)?.parent;
                }
                if remove {
                    doomed.push(candidate).map_err(|_| Error::Capacity)?;
                }
            }
        }
        loop {
            let before = doomed.len();
            for (index, s) in self.store.staging.iter().enumerate() {
                if let Some(n) = &s.node {
                    let candidate = LayerId::new(index, s.generation);
                    if !doomed.contains(&candidate)
                        && (doomed.contains(&n.owner)
                            || n.parent.is_some_and(|p| doomed.contains(&p)))
                    {
                        doomed.push(candidate).map_err(|_| Error::Capacity)?;
                    }
                }
            }
            if doomed.len() == before {
                break;
            }
        }
        while !doomed.is_empty() {
            let i = doomed
                .iter()
                .position(|id| node(&self.store.staging, *id).is_ok_and(|n| n.first.is_none()))
                .ok_or(Error::Cycle)?;
            let id = doomed.swap_remove(i);
            self.unlink(id)?;
            let slot = &mut self.store.staging[id.index()];
            slot.node = None;
            slot.generation = slot
                .generation
                .checked_add(1)
                .expect("layer generation exhausted");
        }
        Ok(())
    }
    pub fn commit(self) -> Result<u64, Error> {
        if self.failed {
            return Err(Error::Poisoned);
        }
        for slot in &mut self.store.staging {
            if let Some(n) = &mut slot.node {
                n.interest =
                    n.properties.bindings.tap.is_some() || n.properties.bindings.pan.is_some();
            }
        }
        for index in 0..self.store.staging.len() {
            let s = &self.store.staging[index];
            if let Some(n) = &s.node {
                let p = &n.properties;
                if p.bounds.size.width > i32::MAX as u32 || p.bounds.size.height > i32::MAX as u32 {
                    return Err(Error::InvalidGeometry);
                }
                let mut x = i64::from(p.bounds.top_left.x);
                let mut y = i64::from(p.bounds.top_left.y);
                let mut ancestor = n.parent;
                while let Some(id) = ancestor {
                    let a = node(&self.store.staging, id)?;
                    x += i64::from(a.properties.bounds.top_left.x);
                    y += i64::from(a.properties.bounds.top_left.y);
                    ancestor = a.parent;
                }
                let outset = i64::from(p.input_outset);
                for (origin, size) in [(x, p.bounds.size.width), (y, p.bounds.size.height)] {
                    if origin - outset < i64::from(i32::MIN)
                        || origin + i64::from(size) + outset > i64::from(i32::MAX)
                        || i64::from(size) + outset * 2 > i64::from(i32::MAX)
                    {
                        return Err(Error::InvalidGeometry);
                    }
                }
                let mut parent = n.parent;
                if n.interest {
                    while let Some(id) = parent {
                        let n = node_mut(&mut self.store.staging, id)?;
                        n.interest = true;
                        parent = n.parent;
                    }
                }
            }
        }
        if self.store.live != self.store.staging {
            core::mem::swap(&mut self.store.live, &mut self.store.staging);
            self.store.committed = self
                .store
                .committed
                .checked_add(1)
                .expect("layer revision exhausted");
            if let Some(c) = self.store.contact {
                let invalid = |binding: Option<(LayerId, Handler)>, pan: bool| {
                    binding.is_some_and(|(id, h)| {
                        !self.store.connected(id)
                            || node(&self.store.live, id).is_ok_and(|n| {
                                if pan {
                                    n.properties.bindings.pan != Some(h)
                                } else {
                                    n.properties.bindings.tap != Some(h)
                                }
                            })
                    })
                };
                if invalid(c.tap, false) || invalid(c.pan, true) {
                    self.store.cancel();
                }
            }
        }
        Ok(self.store.committed)
    }
}

#[cfg(test)]
mod tests;

fn displacement(to: Point, from: Point) -> Point {
    Point::new(to.x.saturating_sub(from.x), to.y.saturating_sub(from.y))
}
