//! Shared page frame, Back gesture, and bounded content rendering.

use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle};

use crate::{TouchEvent, input::TapGesture};

use super::{Button, Header, PageInfo, PageSet, Presentation, Style};

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
    layout: ShellLayout,
    gesture: TapGesture<()>,
}

impl Shell {
    pub const fn new(layout: ShellLayout) -> Self {
        Self {
            layout,
            gesture: TapGesture::new(),
        }
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

    pub(crate) fn touch<Id>(
        &mut self,
        event: TouchEvent,
        page: PageInfo<Id>,
        can_return: bool,
    ) -> bool {
        let back = self.back_control(page, can_return);
        self.gesture
            .update(event, |point| {
                back.filter(|control| control.hit(point)).map(|_| ())
            })
            .is_some()
    }

    pub(crate) fn cancel(&mut self) {
        self.gesture.cancel();
    }

    pub(crate) fn render<P: PageSet, D: DrawTarget<Color = Gray4>>(
        &self,
        pages: &P,
        info: PageInfo<P::Id>,
        can_return: bool,
        display: &mut D,
        style: &Style,
    ) -> Result<(), D::Error> {
        let mut display = display.clipped(&self.layout.viewport);
        display.clear(style.background)?;
        if info.presentation == Presentation::FullScreen {
            return pages.draw_page(info.id, &mut display);
        }
        pages.draw_page(info.id, &mut display.clipped(&self.layout.content))?;
        Header {
            context: info.context,
            title: info.title,
            context_bounds: self.layout.context,
            title_bounds: self.layout.title,
            divider: self.layout.divider,
            controls: self.back_control(info, can_return).as_slice(),
        }
        .render(&mut display, style)
    }
}
