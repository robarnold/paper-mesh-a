//! Gesture recognition for logical controls.

use embedded_graphics::prelude::Point;

use crate::TouchEvent;

pub mod buttons;

/// A tap requires one contact that stays on the same control through release.
/// The caller supplies hit testing and control identities.
pub struct TapGesture<T> {
    armed: Option<(u8, T)>,
    contacts: u16,
}

impl<T> Default for TapGesture<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> TapGesture<T> {
    pub const fn new() -> Self {
        Self {
            armed: None,
            contacts: 0,
        }
    }

    pub fn cancel(&mut self) {
        self.armed = None;
        self.contacts = 0;
    }
}

impl<T: PartialEq> TapGesture<T> {
    /// This method returns an action only for a complete tap.
    /// Release and cancellation clear the armed control.
    pub fn update(
        &mut self,
        event: TouchEvent,
        hit_test: impl FnOnce(Point) -> Option<T>,
    ) -> Option<T> {
        match event {
            TouchEvent::Touch(contact) => {
                let Some(bit) = 1_u16.checked_shl(u32::from(contact.id)) else {
                    self.cancel();
                    return None;
                };
                if self.contacts == 0 {
                    self.armed = hit_test(contact.position).map(|target| (contact.id, target));
                } else if self.contacts != bit
                    || self.armed.as_ref().is_some_and(|(_, target)| {
                        hit_test(contact.position).as_ref() != Some(target)
                    })
                {
                    self.armed = None;
                }
                self.contacts |= bit;
                None
            }
            TouchEvent::Release(contact) => {
                let Some(bit) = 1_u16.checked_shl(u32::from(contact.id)) else {
                    self.cancel();
                    return None;
                };
                self.contacts &= !bit;
                let armed = self.armed.take();
                if let Some((id, target)) = armed
                    && id == contact.id
                    && self.contacts == 0
                    && hit_test(contact.position).as_ref() == Some(&target)
                {
                    return Some(target);
                }
                None
            }
            TouchEvent::Cancel => {
                self.cancel();
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(point: Point) -> Option<i32> {
        (point.x >= 0).then_some(point.x)
    }

    #[test]
    fn movement_cannot_rearm_a_tap_and_other_contacts_cancel_it() {
        let mut tap = TapGesture::new();
        let point = Point::new(1, 0);
        tap.update(TouchEvent::touch(3, point), target);
        tap.update(TouchEvent::touch(3, Point::new(2, 0)), target);
        tap.update(TouchEvent::touch(3, point), target);
        assert_eq!(tap.update(TouchEvent::release(3, point), target), None);
        tap.update(TouchEvent::touch(3, point), target);
        tap.update(TouchEvent::touch(9, point), target);
        tap.update(TouchEvent::release(9, point), target);
        assert_eq!(tap.update(TouchEvent::release(3, point), target), None);
        tap.update(TouchEvent::touch(9, point), target);
        tap.update(TouchEvent::touch(9, point), target);
        assert_eq!(tap.update(TouchEvent::release(9, point), target), Some(1));
    }

    #[test]
    fn a_tap_requires_the_same_target_and_consumes_the_press() {
        let mut tap = TapGesture::new();
        let a = Point::new(1, 0);
        let b = Point::new(2, 0);
        assert_eq!(tap.update(TouchEvent::release(0, a), target), None);
        assert_eq!(tap.update(TouchEvent::touch(0, a), target), None);
        assert_eq!(tap.update(TouchEvent::release(0, b), target), None);
        assert_eq!(tap.update(TouchEvent::release(0, a), target), None);
        tap.update(TouchEvent::touch(0, a), target);
        assert_eq!(tap.update(TouchEvent::release(0, a), target), Some(1));
        assert_eq!(tap.update(TouchEvent::release(0, a), target), None);
    }

    #[test]
    fn cancellation_and_presses_outside_controls_cannot_activate() {
        let mut tap = TapGesture::new();
        let inside = Point::new(1, 0);
        let outside = Point::new(-1, 0);
        tap.update(TouchEvent::touch(0, inside), target);
        assert_eq!(tap.update(TouchEvent::Cancel, target), None);
        assert_eq!(tap.update(TouchEvent::release(0, inside), target), None);
        tap.update(TouchEvent::touch(0, outside), target);
        assert_eq!(tap.update(TouchEvent::release(0, inside), target), None);
        tap.update(TouchEvent::touch(0, inside), target);
        assert_eq!(tap.update(TouchEvent::release(0, outside), target), None);
        tap.update(TouchEvent::touch(0, inside), target);
        tap.cancel();
        assert_eq!(tap.update(TouchEvent::release(0, inside), target), None);
    }
}
