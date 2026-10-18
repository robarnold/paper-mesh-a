//! Gesture recognition for logical controls.

use embedded_graphics::prelude::Point;

use crate::TouchEvent;

/// A tap requires press and release events on the same control.
/// The caller supplies hit testing and control identities.
pub struct TapGesture<T> {
    armed: Option<T>,
}

impl<T> Default for TapGesture<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> TapGesture<T> {
    pub const fn new() -> Self {
        Self { armed: None }
    }

    pub fn cancel(&mut self) {
        self.armed = None;
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
            TouchEvent::Down(point) => {
                self.armed = hit_test(point);
                None
            }
            TouchEvent::Up(point) => {
                let armed = self.armed.take();
                if armed == hit_test(point) {
                    armed
                } else {
                    None
                }
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
    fn a_tap_requires_the_same_target_and_consumes_the_press() {
        let mut tap = TapGesture::new();
        let a = Point::new(1, 0);
        let b = Point::new(2, 0);
        assert_eq!(tap.update(TouchEvent::Up(a), target), None);
        assert_eq!(tap.update(TouchEvent::Down(a), target), None);
        assert_eq!(tap.update(TouchEvent::Up(b), target), None);
        assert_eq!(tap.update(TouchEvent::Up(a), target), None);
        tap.update(TouchEvent::Down(a), target);
        assert_eq!(tap.update(TouchEvent::Up(a), target), Some(1));
        assert_eq!(tap.update(TouchEvent::Up(a), target), None);
    }

    #[test]
    fn cancellation_and_presses_outside_controls_cannot_activate() {
        let mut tap = TapGesture::new();
        let inside = Point::new(1, 0);
        let outside = Point::new(-1, 0);
        tap.update(TouchEvent::Down(inside), target);
        assert_eq!(tap.update(TouchEvent::Cancel, target), None);
        assert_eq!(tap.update(TouchEvent::Up(inside), target), None);
        tap.update(TouchEvent::Down(outside), target);
        assert_eq!(tap.update(TouchEvent::Up(inside), target), None);
        tap.update(TouchEvent::Down(inside), target);
        assert_eq!(tap.update(TouchEvent::Up(outside), target), None);
        tap.update(TouchEvent::Down(inside), target);
        tap.cancel();
        assert_eq!(tap.update(TouchEvent::Up(inside), target), None);
    }
}
