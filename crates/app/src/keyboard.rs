//! Diagnostic page that connects the soft keyboard to a single text field.
//!
//! # Design choices
//!
//! - Apply edits on release so typing can continue during a text refresh.
//! - After a layout change, wait for presentation and a fresh contact so each tap matches
//!   the visible labels.
//!
//! See the [text input contract](../../../docs/platform/text-input.md).

use crate::{pages::Route, screen::STYLE};
use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle, text::Text};
use paper_mesh_platform::{
    TouchEvent,
    ui::{Page, PageEvent, PageInfo, SoftKeyboard, TextField, TextRole},
};

const KEYS: Rectangle = Rectangle::new(Point::new(28, 600), Size::new(484, 320));
const FIELD: Rectangle = Rectangle::new(Point::new(28, 210), Size::new(484, 58));

#[derive(Clone, Copy, PartialEq)]
enum Layout {
    Pending,
    Submitted,
    Visible,
}

pub(crate) struct KeyboardScreen {
    keyboard: SoftKeyboard,
    field: TextField<64>,
    layout: Layout,
    blocked_contacts: u16,
}

impl KeyboardScreen {
    pub const fn new() -> Self {
        Self {
            keyboard: SoftKeyboard::new(KEYS),
            field: TextField::new(),
            layout: Layout::Pending,
            blocked_contacts: 0,
        }
    }

    pub fn submitted(&mut self) {
        if self.layout == Layout::Pending {
            self.layout = Layout::Submitted;
        }
    }

    pub fn completed(&mut self) {
        if self.layout == Layout::Submitted {
            self.layout = Layout::Visible;
        }
    }

    pub fn invalidate(&mut self) {
        self.layout = Layout::Pending;
        self.keyboard.handle_touch(TouchEvent::Cancel);
    }
}

impl Page for KeyboardScreen {
    type Id = Route;
    fn page_info(&self) -> PageInfo<Route> {
        Route::Keyboard.info()
    }
    fn enter(&mut self) {
        *self = Self::new();
    }
    fn leave(&mut self) {}
    fn handle_touch(&mut self, event: TouchEvent) -> PageEvent<Route> {
        if self.layout != Layout::Visible || self.blocked_contacts != 0 {
            match event {
                TouchEvent::Touch(c) => {
                    self.blocked_contacts |= 1u16.checked_shl(c.id.into()).unwrap_or(0)
                }
                TouchEvent::Release(c) => {
                    self.blocked_contacts &= !1u16.checked_shl(c.id.into()).unwrap_or(0)
                }
                TouchEvent::Cancel => self.blocked_contacts = 0,
            }
            return PageEvent::None;
        }
        let update = self.keyboard.handle_touch(event);
        let edited = update.input.is_some_and(|input| self.field.apply(input));
        if update.layout_changed {
            self.layout = Layout::Pending;
        }
        PageEvent::changed(edited || update.layout_changed)
    }

    fn render_page<D: DrawTarget<Color = Gray4>>(&self, target: &mut D) -> Result<(), D::Error> {
        Text::new("Message", Point::new(28, 190), STYLE.text(TextRole::Body)).draw(target)?;
        self.field.render(target, FIELD, &STYLE)?;
        self.keyboard.render(target, &STYLE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tap(screen: &mut KeyboardScreen, point: Point) {
        screen.handle_touch(TouchEvent::touch(0, point));
        screen.handle_touch(TouchEvent::release(0, point));
    }

    #[test]
    fn layout_changes_require_a_visible_frame_and_fresh_contact() {
        let mut screen = KeyboardScreen::new();
        let shift = Point::new(76, 800);
        let a = Point::new(76, 720);
        screen.submitted();
        screen.completed();
        tap(&mut screen, shift);
        screen.handle_touch(TouchEvent::touch(0, a));
        screen.completed(); // Completion of an older frame.
        screen.submitted();
        screen.completed();
        screen.handle_touch(TouchEvent::touch(0, a));
        screen.handle_touch(TouchEvent::release(0, a));
        assert!(screen.field.value.is_empty());
        tap(&mut screen, a);
        assert_eq!(screen.field.value.as_str(), "A");
        screen.submitted();
        screen.completed();
        screen.handle_touch(TouchEvent::touch(0, a));
        screen.handle_touch(TouchEvent::touch(0, a));
        assert_eq!(screen.field.value.as_str(), "A");
        screen.handle_touch(TouchEvent::release(0, a));
        tap(&mut screen, a);
        assert_eq!(screen.field.value.as_str(), "Aaa");
        screen.handle_touch(TouchEvent::touch(0, a));
        screen.handle_touch(TouchEvent::touch(0, Point::zero()));
        screen.handle_touch(TouchEvent::release(0, a));
        assert_eq!(screen.field.value.as_str(), "Aaa");
    }

    #[test]
    fn letter_tap_consumes_shift_at_capacity() {
        let mut screen = KeyboardScreen::new();
        screen.submitted();
        screen.completed();
        let a = Point::new(76, 720);
        for _ in 0..64 {
            tap(&mut screen, a);
        }
        tap(&mut screen, Point::new(76, 800));
        screen.submitted();
        screen.completed();
        tap(&mut screen, a);
        assert_eq!(screen.field.value.len(), 64);
        screen.submitted();
        screen.completed();
        tap(&mut screen, Point::new(463, 800));
        assert_eq!(screen.field.value.len(), 63);
        tap(&mut screen, a);
        assert!(screen.field.value.ends_with('a'));
    }
}
