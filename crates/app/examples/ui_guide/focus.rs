//! Diagnostic page that transfers system keyboard focus between two text fields.
//!
//! # Design choices
//!
//! - Apply edits on release so typing can continue during a text refresh.
//! - Keep values in the page while the system owns keyboard state and focus sessions.
//!
//! See the [text input contract](../../../../docs/platform/text-input.md).

use super::{LAYOUT, STYLE};
use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle, text::Text};
use paper_mesh_platform::ui::EnabledState;
use paper_mesh_platform::ui::{Page, PageInfo, TextField, TextRole};

const KEYS: Rectangle = Rectangle::new(Point::new(28, 600), Size::new(484, 320));
const FIELD: Rectangle = Rectangle::new(Point::new(28, 210), Size::new(484, 58));

pub struct FocusPage {
    field: TextField<64>,
    second: TextField<64>,
    focused: u16,
    elements: paper_mesh_platform::ui::elements::Elements<Self, u16, 3, 0>,
}
impl FocusPage {
    pub const fn new() -> Self {
        Self {
            field: TextField::new(),
            second: TextField::new(),
            focused: 0,
            elements: paper_mesh_platform::ui::elements::Elements::new(LAYOUT.viewport),
        }
    }
}
impl Page for FocusPage {
    type Id = u8;
    type Elements = paper_mesh_platform::ui::elements::Elements<Self, u16, 3, 0>;
    fn elements(&self) -> &Self::Elements {
        &self.elements
    }
    fn elements_mut(&mut self) -> &mut Self::Elements {
        &mut self.elements
    }
    fn page_info(&self) -> PageInfo<u8> {
        PageInfo {
            id: 0,
            title: "Text entry",
            context: "Focus",
            presentation: paper_mesh_platform::ui::Presentation::Standard,
            navigation_enabled: true,
        }
    }
    fn did_enter(&mut self) {
        self.field = TextField::new();
        self.second = TextField::new();
        self.focused = 0;
    }
    fn sync_elements(&mut self) {
        self.elements
            .surface([
                (FIELD, 0, EnabledState::Enabled),
                (
                    Rectangle::new(Point::new(28, 330), Size::new(484, 58)),
                    1,
                    EnabledState::Enabled,
                ),
            ])
            .expect("field surface fits");
    }
    fn configure_handlers(&mut self) {
        self.elements.default_callback(|page, context, field| {
            page.focused = field;
            context.changed();
        });
    }
    fn text_focus(&self) -> Option<paper_mesh_platform::ui::TextFocus> {
        Some(paper_mesh_platform::ui::TextFocus {
            field: self.focused,
            keyboard: KEYS,
        })
    }
    fn text_input(&mut self, field: u16, input: paper_mesh_platform::ui::TextInput) -> bool {
        if field == 0 {
            self.field.apply(input)
        } else if field == 1 {
            self.second.apply(input)
        } else {
            false
        }
    }
    fn draw_custom<D: DrawTarget<Color = Gray4>>(
        &self,
        _: u16,
        target: &mut D,
    ) -> Result<(), D::Error> {
        Text::new(
            if self.focused == 0 {
                "Message (active)"
            } else {
                "Message"
            },
            Point::new(28, 190),
            STYLE.text(TextRole::Body),
        )
        .draw(target)?;
        self.field.render(target, FIELD, &STYLE)?;
        Text::new(
            if self.focused == 1 {
                "Label (active)"
            } else {
                "Label"
            },
            Point::new(28, 310),
            STYLE.text(TextRole::Body),
        )
        .draw(target)?;
        self.second.render(
            target,
            Rectangle::new(Point::new(28, 330), Size::new(484, 58)),
            &STYLE,
        )
    }
}

impl Default for FocusPage {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paper_mesh_platform::{
        TouchEvent,
        ui::{Navigator, Shell, SoftKeyboard},
    };

    // The preview transfers focus without moving earlier text into the second field.
    #[test]
    fn preview_edits_only_the_focused_field() {
        let mut ui = Navigator::<_, 0>::launch(0, Shell::new(LAYOUT), FocusPage::new());
        let (_, key) = SoftKeyboard::new(KEYS).keys().next().unwrap();
        let tap = |ui: &mut Navigator<FocusPage, 0>, position| {
            ui.touch(TouchEvent::touch(0, position)).unwrap();
            ui.touch(TouchEvent::release(0, position)).unwrap();
        };
        tap(&mut ui, key.center());
        assert_eq!(ui.pages().field.value.as_str(), "q");
        tap(&mut ui, Point::new(80, 350));
        tap(&mut ui, key.center());
        assert_eq!(ui.pages().field.value.as_str(), "q");
        assert_eq!(ui.pages().second.value.as_str(), "q");
    }
}
