//! Shared page frame and Back control.
//!
//! # Design choices
//!
//! - Clip standard pages to the content region so drawing cannot cover navigation.
//!   Give full-screen pages the entire viewport so they can use the header area.
//! - Use the same Back control for rendering and hit testing so a disabled Back
//!   button cannot complete a tap.
//!
//! See the [pages and shells contract](../../../../docs/platform/toolkit.md#pages-and-shells).

use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle};

use super::{Button, Header, PageInfo, Presentation, Style};

#[derive(Clone, Copy)]
pub struct ShellLayout {
    pub viewport: Rectangle,
    pub content: Rectangle,
    pub context: Rectangle,
    pub title: Rectangle,
    pub divider: Rectangle,
    pub back: Rectangle,
}

pub struct Shell {
    pub(crate) layout: ShellLayout,
}

impl Shell {
    pub const fn new(layout: ShellLayout) -> Self {
        Self { layout }
    }

    pub(crate) fn back_control<Id>(
        &self,
        page: PageInfo<Id>,
        can_return: bool,
    ) -> Option<Button<'static>> {
        (can_return && page.presentation == Presentation::Standard).then_some(Button {
            bounds: self.layout.back,
            label: "Back",
            selected: false,
            enabled: page.navigation_enabled,
        })
    }

    pub(crate) fn render<Id, D: DrawTarget<Color = Gray4>>(
        &self,
        info: PageInfo<Id>,
        can_return: bool,
        display: &mut D,
        style: &Style,
    ) -> Result<(), D::Error> {
        if info.presentation == Presentation::FullScreen {
            return Ok(());
        }
        Header {
            context: info.context,
            title: info.title,
            context_bounds: self.layout.context,
            title_bounds: self.layout.title,
            divider: self.layout.divider,
            controls: self.back_control(info, can_return).as_slice(),
        }
        .render(display, style)
    }
}
