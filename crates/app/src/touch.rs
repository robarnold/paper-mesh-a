//! Touch diagnostics with a retained tap counter.
//!
//! # Design choices
//!
//! - Retain the count across page visits but cancel the pending gesture so a release
//!   after returning cannot count as a new tap.
//!
//! See the [touch check](../../../docs/application.md#touch-check).

use crate::screen::{STYLE, button};
use paper_mesh_platform::ui::EnabledState;
use paper_mesh_platform::ui::TextRole;

use core::fmt::Write;
use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle, text::Text};
use heapless::String;

pub const TAP_TARGET: Rectangle = Rectangle::new(Point::new(48, 380), Size::new(444, 200));

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Count,
}

pub struct TouchScreen {
    open: bool,
    taps: u32,
    pub(crate) elements: paper_mesh_platform::ui::elements::Elements<TouchScreen, Action, 8, 0>,
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
            elements: paper_mesh_platform::ui::elements::Elements::new(
                crate::screen::LAYOUT.viewport,
            ),
        }
    }
    pub fn open(&mut self) {
        self.open = true;
    }
    pub fn close(&mut self) {
        self.open = false;
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn taps(&self) -> u32 {
        self.taps
    }

    pub(crate) fn sync_nodes(&mut self) {
        self.elements
            .surface([(TAP_TARGET, Action::Count, EnabledState::Enabled)])
            .expect("fixed surface fits element capacity");
    }
    pub(crate) fn activate(&mut self, _: Action) -> bool {
        self.taps = self.taps.saturating_add(1);
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
