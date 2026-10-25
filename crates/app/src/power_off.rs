//! Explicit power-off controls and restart instructions from the backend.

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Gray4,
    prelude::*,
    primitives::Rectangle,
    text::Text,
};
use paper_mesh_platform::{TouchEvent, input::TapGesture, power_off::PowerOffError, ui::button};

use crate::BACK_TARGET;

pub const START: Rectangle = Rectangle::new(Point::new(28, 420), Size::new(484, 64));

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Back,
    Start,
}

pub struct PowerOffScreen {
    pub open: bool,
    pending: bool,
    request: bool,
    error: Option<PowerOffError>,
    restart_hint: Option<&'static str>,
    gesture: TapGesture<Action>,
}

impl Default for PowerOffScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerOffScreen {
    pub const fn new() -> Self {
        Self {
            open: false,
            pending: false,
            request: false,
            error: None,
            restart_hint: None,
            gesture: TapGesture::new(),
        }
    }

    pub fn set_restart_hint(&mut self, hint: &'static str) {
        self.restart_hint = Some(hint);
    }

    pub fn take_request(&mut self) -> bool {
        core::mem::take(&mut self.request)
    }

    pub fn request(&mut self) {
        if self.restart_hint.is_some() && !self.pending {
            self.open = true;
            self.error = None;
            self.pending = true;
            self.request = true;
            self.gesture.cancel();
        }
    }

    pub fn finish(&mut self, error: PowerOffError) {
        self.pending = false;
        self.error = Some(error);
        self.gesture.cancel();
    }

    pub fn error(&self) -> Option<PowerOffError> {
        self.error
    }

    pub fn touch(&mut self, event: TouchEvent) -> bool {
        if self.pending {
            return false;
        }
        let action = self.gesture.update(event, |point| {
            if BACK_TARGET.contains(point) {
                Some(Action::Back)
            } else if self.restart_hint.is_some() && START.contains(point) {
                Some(Action::Start)
            } else {
                None
            }
        });
        match action {
            Some(Action::Back) => self.open = false,
            Some(Action::Start) => {
                self.request();
            }
            None => return false,
        }
        true
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        display.clear(Gray4::WHITE)?;
        button(display, BACK_TARGET, "Back", false, !self.pending)?;
        for (y, text) in [
            (58, "POWER OFF"),
            (170, "Disconnect USB and external power."),
            (210, "Then press Power off."),
            (280, "The screen image remains without power."),
            (320, "To restart:"),
            (
                354,
                self.restart_hint.unwrap_or("Power off is unavailable."),
            ),
        ] {
            line(display, y, text)?;
        }
        button(
            display,
            START,
            if self.pending {
                "Power off requested"
            } else {
                "Power off"
            },
            false,
            !self.pending && self.restart_hint.is_some(),
        )?;
        if let Some(error) = self.error {
            line(display, 540, error.label())?;
        }
        Ok(())
    }
}

fn line<D: DrawTarget<Color = Gray4>>(display: &mut D, y: i32, text: &str) -> Result<(), D::Error> {
    Text::new(
        text,
        Point::new(28, y),
        MonoTextStyle::new(&FONT_10X20, Gray4::BLACK),
    )
    .draw(display)?;
    Ok(())
}
