//! Navigation between retained pages, including return history and input routing.
//!
//! # Design choices
//!
//! - Record the caller on a bounded stack so Back retraces visits even when different
//!   pages open the same destination.
//! - Reject an open request when history is full before leaving the current page.
//!   This preserves both its lifecycle state and the existing return path.
//! - Cancel pending gestures on transition so a finger already on the screen cannot
//!   activate a control on the destination page.
//!
//! See the [pages and shells contract](../../../../docs/platform/toolkit.md#pages-and-shells).

use embedded_graphics::{pixelcolor::Gray4, prelude::*};
use heapless::Vec;

use crate::{
    TouchEvent,
    composition::{Bindings, Delivery, Drawing, Gesture, Handler, LayerId, Layers, Properties},
};

use super::{Button, Navigation, Page, PageEvent, PageInfo, PageRuntime, PageSet, Shell, Style};

#[cfg(test)]
#[path = "navigation_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition<Id> {
    pub from: Id,
    pub to: Id,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationError {
    HistoryFull,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Update<Id> {
    pub changed: bool,
    pub transition: Option<Transition<Id>>,
}

impl<Id> Update<Id> {
    const NONE: Self = Self {
        changed: false,
        transition: None,
    };
}

/// One launched shell with retained pages and a bounded stack of return destinations.
pub struct Navigator<P: PageSet, const CAPACITY: usize> {
    shell: Shell,
    pages: P,
    home: P::Id,
    current: P::Id,
    history: Vec<P::Id, CAPACITY>,
    contacts: u16,
    suppressed: bool,
    launched: bool,
    layers: Layers<64>,
    roots: Vec<(P::Id, LayerId), 1>,
    shell_root: Option<LayerId>,
    content_root: Option<LayerId>,
    back_layer: Option<LayerId>,
    keyboard: super::SystemKeyboard,
    keyboard_root: Option<LayerId>,
    focus: Option<(P::Id, super::TextFocus)>,
}

impl<P: PageSet, const CAPACITY: usize> Navigator<P, CAPACITY> {
    /// Enter the root page once before input or rendering can reach the shell.
    pub fn launch(home: P::Id, shell: Shell, pages: P) -> Self {
        let mut navigator = Self::unlaunched(home, shell, pages);
        navigator.launch_in_place();
        navigator
    }

    /// Prepare static storage. Call `launch_in_place` before navigation, input, or rendering.
    pub const fn unlaunched(home: P::Id, shell: Shell, pages: P) -> Self {
        Self {
            shell,
            pages,
            home,
            current: home,
            history: Vec::new(),
            contacts: 0,
            suppressed: false,
            launched: false,
            layers: Layers::new(),
            roots: Vec::new(),
            shell_root: None,
            content_root: None,
            back_layer: None,
            keyboard: super::SystemKeyboard::new(),
            keyboard_root: None,
            focus: None,
        }
    }

    pub fn launch_in_place(&mut self) {
        assert!(!self.launched, "navigator is already launched");
        assert!(
            self.pages.info(self.home).id == self.home,
            "root page identity must match"
        );
        self.pages.enter_page(self.home);
        self.launched = true;
        self.prepare();
    }

    pub fn current(&self) -> P::Id {
        self.current
    }

    pub fn return_to(&self) -> Option<P::Id> {
        self.history.last().copied()
    }

    /// Return all callers, oldest first, without the current page.
    /// The last entry is the next Back destination.
    pub fn history(&self) -> &[P::Id] {
        self.history.as_slice()
    }

    pub fn pages(&self) -> &P {
        &self.pages
    }

    pub fn pages_mut(&mut self) -> &mut P {
        &mut self.pages
    }

    pub fn page_info(&self) -> PageInfo<P::Id> {
        assert!(self.launched, "navigator must be launched");
        let info = self.pages.info(self.current);
        assert!(info.id == self.current, "active page identity must match");
        info
    }

    pub fn back_control(&self) -> Option<Button<'static>> {
        self.shell
            .back_control(self.page_info(), self.return_to().is_some())
    }

    /// Rejected requests leave the destination, history, and lifecycle unchanged.
    pub fn navigate(
        &mut self,
        navigation: Navigation<P::Id>,
    ) -> Result<Option<Transition<P::Id>>, NavigationError> {
        if !self.page_info().navigation_enabled {
            return Ok(None);
        }
        let to = match navigation {
            Navigation::Home => self.home,
            Navigation::Back => match self.return_to() {
                Some(id) => id,
                None => return Ok(None),
            },
            Navigation::Open(id) => id,
        };
        if to == self.current {
            self.cancel_input();
            return Ok(None);
        }
        assert!(
            self.pages.info(to).id == to,
            "destination identity must match"
        );
        if to == self.home {
            self.history.clear();
        } else if navigation == Navigation::Back {
            self.history.pop();
        } else {
            self.history
                .push(self.current)
                .map_err(|_| NavigationError::HistoryFull)?;
        }
        self.cancel_input();
        let from = self.current;
        self.pages.leave_page(from);
        self.current = to;
        self.pages.enter_page(to);
        self.prepare();
        Ok(Some(Transition { from, to }))
    }

    fn cancel_input(&mut self) {
        if let Some(delivery) = self.layers.touch(TouchEvent::Cancel) {
            self.deliver_cancellation(delivery);
        }
        self.suppressed = self.contacts != 0;
    }

    fn deliver_cancellation(&mut self, delivery: Delivery) {
        if !self.layers.contains_handler(delivery.handler) {
            return;
        }
        if let Some(&(id, _)) = self
            .roots
            .iter()
            .find(|(_, root)| *root == delivery.handler.owner)
        {
            // Cancellation releases page gesture state; it must not initiate another transition.
            if self
                .pages
                .activate_page(id, delivery.handler.slot, delivery.gesture)
                != PageEvent::None
            {
                self.pages.invalidate_page(id);
            }
        }
    }

    /// Route input through the shared frame, then the active page.
    pub fn touch(&mut self, event: TouchEvent) -> Result<Update<P::Id>, NavigationError> {
        match event {
            TouchEvent::Touch(contact) | TouchEvent::Release(contact) => {
                let Some(bit) = 1_u16.checked_shl(u32::from(contact.id)) else {
                    self.cancel_input();
                    return Ok(Update::NONE);
                };
                if matches!(event, TouchEvent::Touch(_)) {
                    self.contacts |= bit;
                } else {
                    self.contacts &= !bit;
                }
                if self.suppressed {
                    self.suppressed = self.contacts != 0;
                    return Ok(Update::NONE);
                }
            }
            TouchEvent::Cancel => {
                self.contacts = 0;
                self.suppressed = false;
            }
        }
        self.prepare();
        let event = match self.layers.touch(event) {
            Some(delivery)
                if Some(delivery.handler.owner) == self.shell_root
                    && delivery.gesture == Gesture::Tap =>
            {
                PageEvent::Navigate(Navigation::Back)
            }
            Some(delivery)
                if Some(delivery.handler.owner) == self.keyboard_root
                    && delivery.gesture == Gesture::Tap =>
            {
                self.keyboard
                    .activate(delivery.handler.slot, delivery.gesture);
                if let Some((session, input)) = self.keyboard.take_input() {
                    self.deliver_text(session, input);
                }
                self.prepare();
                PageEvent::Changed
            }
            Some(delivery) => {
                let result =
                    self.pages
                        .activate_page(self.current, delivery.handler.slot, delivery.gesture);
                if result != PageEvent::None {
                    self.pages.invalidate_page(self.current);
                }
                self.prepare();
                result
            }
            _ => PageEvent::None,
        };
        match event {
            PageEvent::None => Ok(Update::NONE),
            PageEvent::Changed => Ok(Update {
                changed: true,
                transition: None,
            }),
            PageEvent::Navigate(request) => {
                let transition = self.navigate(request)?;
                Ok(Update {
                    changed: transition.is_some(),
                    transition,
                })
            }
        }
    }

    pub fn input_ready(&self) -> bool {
        self.launched
    }
    pub fn text_session(&self) -> Option<super::TextSession> {
        self.focus.map(|_| self.keyboard.session())
    }
    pub fn deliver_text(&mut self, session: super::TextSession, input: super::TextInput) -> bool {
        if session != self.keyboard.session() {
            return false;
        }
        let Some((id, focus)) = self.focus else {
            return false;
        };
        let changed = self.pages.text_input_page(id, focus.field, input);
        if changed {
            self.pages.invalidate_page(id);
        }
        changed
    }
    pub fn layers(&self) -> &Layers<64> {
        &self.layers
    }
    pub fn prepare(&mut self) {
        let viewport = self.shell.layout.viewport;
        let system = self.layers.initialize(viewport).expect("system root fits");
        let info = self.page_info();
        let back = self.back_control();
        let focus = self
            .pages
            .text_focus_page(self.current)
            .map(|focus| (self.current, focus));
        if focus != self.focus {
            if let Some(delivery) = self.layers.touch(TouchEvent::Cancel) {
                self.deliver_cancellation(delivery);
            }
            self.focus = focus;
            self.keyboard.focus(focus.map_or(
                embedded_graphics::primitives::Rectangle::zero(),
                |(_, f)| f.keyboard,
            ));
        }
        let mut tx = self.layers.begin(system).unwrap();
        // Retain page models and elements, but reclaim inactive layer allocations to bound RAM.
        for &(id, root) in &self.roots {
            if id != self.current {
                tx.destroy(root).expect("registered owner root");
            }
        }
        self.roots.retain(|(id, _)| *id == self.current);
        if self.shell_root.is_none() {
            self.shell_root = Some(tx.create_root().unwrap());
            self.content_root = Some(tx.create().unwrap());
            self.back_layer = Some(
                tx.within(self.shell_root.unwrap(), |tx| tx.create())
                    .unwrap(),
            );
        }
        if self.focus.is_some() && self.keyboard_root.is_none() {
            self.keyboard_root = Some(tx.create_root().expect("keyboard root fits"));
        } else if self.focus.is_none()
            && let Some(root) = self.keyboard_root.take()
        {
            tx.destroy(root).expect("registered keyboard root");
        }
        let root = if let Some((_, root)) = self.roots.iter().find(|(id, _)| *id == self.current) {
            *root
        } else {
            let root = tx.create_root().expect("page root fits");
            self.roots
                .push((self.current, root))
                .ok()
                .expect("page registry fits");
            root
        };
        self.pages
            .publish_page(self.current, &mut tx, root)
            .expect("page layers fit");
        let clip = if info.presentation == super::Presentation::FullScreen {
            viewport
        } else {
            self.shell.layout.content
        };
        let mut props = tx.properties(root).unwrap().clone();
        props.bounds.top_left -= clip.top_left;
        tx.set(root, props).unwrap();
        let content = self.content_root.unwrap();
        tx.set(
            content,
            Properties {
                bounds: clip,
                ..Properties::default()
            },
        )
        .unwrap();
        tx.replace_children(content, &[root]).unwrap();
        let shell = self.shell_root.unwrap();
        tx.set(
            shell,
            Properties {
                bounds: viewport,
                drawing: Drawing::Custom(0),
                ..Properties::default()
            },
        )
        .unwrap();
        let back_id = self.back_layer.unwrap();
        tx.set(
            back_id,
            Properties {
                bounds: self.shell.layout.back,
                bindings: Bindings {
                    tap: back.filter(|b| b.enabled).map(|_| Handler {
                        owner: shell,
                        slot: 0,
                        generation: 0,
                    }),
                    pan: None,
                },
                ..Properties::default()
            },
        )
        .unwrap();
        tx.replace_children(shell, &[back_id]).unwrap();
        tx.set(
            system,
            Properties {
                bounds: viewport,
                drawing: Drawing::Solid(Style::GRAYSCALE.background),
                ..Properties::default()
            },
        )
        .unwrap();
        if self.focus.is_some() {
            self.keyboard
                .publish(&mut tx, self.keyboard_root.unwrap())
                .expect("keyboard layers fit");
            tx.replace_children(system, &[content, shell, self.keyboard_root.unwrap()])
                .unwrap();
        } else {
            tx.replace_children(system, &[content, shell]).unwrap();
        }
        tx.commit().expect("valid layer batch");
        while let Some(delivery) = self.layers.take_cancellation() {
            self.deliver_cancellation(delivery);
        }
    }
    pub fn submitted(&mut self) {
        self.layers.submitted();
    }
    pub fn completed(&mut self) {
        self.layers.completed();
    }
    pub fn invalidate(&mut self) {
        self.pages.invalidate_page(self.current);
        self.layers.invalidate();
    }
    pub fn render<D: DrawTarget<Color = Gray4>>(
        &self,
        display: &mut D,
        style: &Style,
    ) -> Result<(), D::Error> {
        self.layers.render(display, |owner, slot, target| {
            if Some(owner) == self.shell_root {
                self.shell
                    .render(self.page_info(), self.return_to().is_some(), target, style)
            } else if Some(owner) == self.keyboard_root {
                Page::draw_custom(&self.keyboard, slot, target)
            } else {
                let id = self
                    .roots
                    .iter()
                    .find(|(_, root)| *root == owner)
                    .expect("live layer owner")
                    .0;
                self.pages.draw_page_custom(id, slot, target)
            }
        })
    }
}
