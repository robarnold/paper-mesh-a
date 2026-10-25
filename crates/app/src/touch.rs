//! Touch diagnostics with a retained tap counter.

use crate::screen::{STYLE, button};
use paper_mesh_platform::ui::TextRole;

use core::fmt::Write;
use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle, text::Text};
use heapless::String;
use paper_mesh_platform::{TouchEvent, input::TapGesture};

pub const TAP_TARGET: Rectangle = Rectangle::new(Point::new(48, 380), Size::new(444, 200));

#[derive(PartialEq)]
enum Action {
    Count,
}

pub struct TouchScreen {
    open: bool,
    taps: u32,
    gesture: TapGesture<Action>,
}

impl Default for TouchScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl TouchScreen {
    pub const fn new() -> Self {
        Self {
            open: false,
            taps: 0,
            gesture: TapGesture::new(),
        }
    }
    pub fn open(&mut self) {
        self.open = true;
        self.gesture.cancel();
    }
    pub fn close(&mut self) {
        self.open = false;
        self.gesture.cancel();
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn taps(&self) -> u32 {
        self.taps
    }

    pub fn touch(&mut self, event: TouchEvent) -> bool {
        match self.gesture.update(event, |point| {
            if TAP_TARGET.contains(point) {
                Some(Action::Count)
            } else {
                None
            }
        }) {
            Some(Action::Count) => self.taps = self.taps.saturating_add(1),
            None => return false,
        }
        true
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        display.clear(STYLE.background)?;
        let style = STYLE.text(TextRole::Body);
        for (y, text) in [
            (150, "Tap inside the target."),
            (190, "Release inside to increase the count."),
            (230, "Move outside to cancel a tap."),
        ] {
            Text::new(text, Point::new(28, y), style).draw(display)?;
        }
        button(display, TAP_TARGET, "Tap to count", true, true)?;
        let mut label = String::<32>::new();
        let _ = write!(label, "Touches: {}", self.taps);
        Text::new(&label, Point::new(28, 650), style).draw(display)?;
        Text::new("The count stays until restart.", Point::new(28, 710), style).draw(display)?;
        Ok(())
    }
}
