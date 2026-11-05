//! Explicit power-off controls and restart instructions from the backend.
//!
//! # Design choices
//!
//! - Require an explicit request so opening the page to read the restart instructions
//!   cannot shut down the device.
//! - Enable the request only when the backend supplies restart instructions so the
//!   user can see how to recover power.
//!
//! See the [power-off policy](../../../docs/application.md#power-off).

use crate::screen::{STYLE, button};
use paper_mesh_platform::ui::EnabledState;
use paper_mesh_platform::ui::TextRole;

use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle, text::Text};
use paper_mesh_platform::power_off::PowerOffError;

pub const START: Rectangle = Rectangle::new(Point::new(28, 420), Size::new(484, 64));

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Start,
}

pub struct PowerOffScreen {
    pub open: bool,
    pending: bool,
    request: bool,
    error: Option<PowerOffError>,
    restart_hint: Option<&'static str>,
    pub(crate) elements: paper_mesh_platform::ui::elements::Elements<PowerOffScreen, Action, 8, 0>,
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
            elements: paper_mesh_platform::ui::elements::Elements::new(
                crate::screen::LAYOUT.viewport,
            ),
        }
    }

    pub fn pending(&self) -> bool {
        self.pending
    }
    pub fn can_start(&self) -> bool {
        !self.pending && self.restart_hint.is_some()
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn set_restart_hint(&mut self, hint: &'static str) {
        self.restart_hint = Some(hint);
    }

    pub fn take_request(&mut self) -> bool {
        core::mem::take(&mut self.request)
    }

    pub fn request(&mut self) {
        if self.can_start() {
            self.open = true;
            self.error = None;
            self.pending = true;
            self.request = true;
        }
    }

    pub fn finish(&mut self, error: PowerOffError) {
        self.pending = false;
        self.error = Some(error);
    }

    pub fn error(&self) -> Option<PowerOffError> {
        self.error
    }

    pub(crate) fn sync_nodes(&mut self) {
        self.elements
            .surface([(START, Action::Start, EnabledState::from(self.can_start()))])
            .expect("fixed surface fits element capacity");
    }
    pub(crate) fn activate(&mut self, action: Action) -> bool {
        match Some(action) {
            Some(Action::Start) => {
                self.request();
            }
            None => return false,
        }
        true
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        display.clear(STYLE.background)?;
        for (y, text) in [
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
            self.can_start(),
        )?;
        if let Some(error) = self.error {
            Text::new(
                error.label(),
                Point::new(28, 540),
                STYLE.text(TextRole::Body),
            )
            .draw(display)?;
        }
        Ok(())
    }
}

fn line<D: DrawTarget<Color = Gray4>>(display: &mut D, y: i32, text: &str) -> Result<(), D::Error> {
    Text::new(text, Point::new(28, y), STYLE.text(TextRole::Body)).draw(display)?;
    Ok(())
}
