//! System-owned keyboard client, composed through ordinary UI elements.
//!
//! # Design choices
//!
//! - Keep focus sessions outside applications so obsolete text cannot reach another field.
//! - Let the touchscreen stack recognize gestures; this client consumes key activations.
//!
//! See [text input](../../../../docs/platform/text-input.md).

use super::EnabledState;
use super::{
    Page, PageEvent, PageInfo, Presentation, SoftKeyboard, TextInput, elements::Elements,
    keyboard::Key,
};
use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextFocus {
    pub field: u16,
    pub keyboard: Rectangle,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSession(pub u64);

pub struct SystemKeyboard {
    pub keyboard: SoftKeyboard,
    elements: Elements<Self, Key, 36, 0>,
    input: Option<TextInput>,
    session: TextSession,
}
impl SystemKeyboard {
    pub const fn new() -> Self {
        Self {
            keyboard: SoftKeyboard::new(Rectangle::zero()),
            elements: Elements::new(Rectangle::zero()),
            input: None,
            session: TextSession(0),
        }
    }
    pub fn focus(&mut self, bounds: Rectangle) {
        self.session.0 = self
            .session
            .0
            .checked_add(1)
            .expect("text session exhausted");
        self.keyboard = SoftKeyboard::new(bounds);
        self.elements.set_viewport(bounds);
        self.elements.invalidate();
        self.elements.bump_handlers();
        self.input = None;
    }
    pub fn session(&self) -> TextSession {
        self.session
    }
    pub fn take_input(&mut self) -> Option<(TextSession, TextInput)> {
        self.input.take().map(|i| (self.session, i))
    }
}
impl Default for SystemKeyboard {
    fn default() -> Self {
        Self::new()
    }
}
impl Page for SystemKeyboard {
    type Id = u8;
    type Elements = Elements<Self, Key, 36, 0>;
    fn elements(&self) -> &Self::Elements {
        &self.elements
    }
    fn elements_mut(&mut self) -> &mut Self::Elements {
        &mut self.elements
    }
    fn page_info(&self) -> PageInfo<u8> {
        PageInfo {
            id: 0,
            title: "Keyboard",
            context: "",
            presentation: Presentation::FullScreen,
            navigation_enabled: true,
        }
    }
    fn sync_elements(&mut self) {
        self.elements
            .surface(
                self.keyboard
                    .keys()
                    .map(|(key, bounds)| (bounds, key, EnabledState::Enabled)),
            )
            .expect("keyboard capacity fits");
    }
    fn configure_handlers(&mut self) {
        self.elements.default_callback(|keyboard, context, key| {
            let update = keyboard.keyboard.activate(key);
            keyboard.input = update.input;
            context.respond(PageEvent::changed(
                update.input.is_some() || update.layout_changed,
            ));
        });
    }
    fn draw_custom<D: DrawTarget<Color = Gray4>>(
        &self,
        _: u16,
        target: &mut D,
    ) -> Result<(), D::Error> {
        self.keyboard.render(target, &super::Style::GRAYSCALE)
    }
}
