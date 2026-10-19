//! Complete contact frames with bounded storage.

use crate::{Contact, TouchEvent};

pub const MAX_CONTACTS: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TouchFrame {
    pub at_ms: u64,
    events: [Option<TouchEvent>; MAX_CONTACTS * 2],
    len: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContactState {
    contacts: [Option<Contact>; MAX_CONTACTS],
}

impl ContactState {
    pub const fn new() -> Self {
        Self {
            contacts: [None; MAX_CONTACTS],
        }
    }
    pub fn contacts(&self) -> impl Iterator<Item = Contact> + '_ {
        self.contacts.iter().flatten().copied()
    }
    pub fn is_empty(&self) -> bool {
        self.contacts.iter().all(Option::is_none)
    }
    pub fn clear(&mut self) {
        self.contacts.fill(None);
    }

    /// Returns false after cancellation or an invalid frame. Partial changes stay private.
    pub fn apply(&mut self, frame: &TouchFrame) -> bool {
        let mut next = *self;
        for event in frame.events() {
            match event {
                TouchEvent::Touch(contact) if contact.id < 16 => {
                    let slot = next
                        .contacts
                        .iter()
                        .position(|c| c.is_some_and(|c| c.id == contact.id))
                        .or_else(|| next.contacts.iter().position(Option::is_none));
                    let Some(slot) = slot else {
                        self.clear();
                        return false;
                    };
                    next.contacts[slot] = Some(contact);
                }
                TouchEvent::Release(contact) if contact.id < 16 => {
                    for slot in &mut next.contacts {
                        if slot.is_some_and(|c| c.id == contact.id) {
                            *slot = None;
                        }
                    }
                }
                _ => {
                    self.clear();
                    return false;
                }
            }
        }
        *self = next;
        true
    }
}

impl TouchFrame {
    pub const fn new(at_ms: u64) -> Self {
        Self {
            at_ms,
            events: [None; MAX_CONTACTS * 2],
            len: 0,
        }
    }
    pub fn single(event: TouchEvent, at_ms: u64) -> Self {
        let mut frame = Self::new(at_ms);
        frame.push(event);
        frame
    }
    pub fn push(&mut self, event: TouchEvent) {
        assert!(self.len < self.events.len());
        self.events[self.len] = Some(event);
        self.len += 1;
    }
    pub fn events(&self) -> impl Iterator<Item = TouchEvent> + '_ {
        self.events[..self.len].iter().flatten().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::prelude::Point;
    #[test]
    fn overflow_cancels_all_contacts_without_partial_state() {
        let mut state = ContactState::new();
        for id in 0..5 {
            assert!(state.apply(&TouchFrame::single(
                TouchEvent::touch(id, Point::new(1, 2)),
                0
            )));
        }
        assert!(!state.apply(&TouchFrame::single(
            TouchEvent::touch(5, Point::new(1, 2)),
            1
        )));
        assert!(state.is_empty());
    }
}
