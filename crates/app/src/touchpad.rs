//! Touchpad application screen and activation policy.

use crate::screen::STYLE;
use paper_mesh_platform::ui::TextRole;

use crate::pages::Route;
use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle, text::Text};
use paper_mesh_platform::touchpad::HostState;
use paper_mesh_platform::{
    TouchEvent,
    ui::{Page, PageEvent, PageInfo, Presentation},
};

pub struct TouchpadPage {
    pub host: HostState,
}

impl Default for TouchpadPage {
    fn default() -> Self {
        Self::new()
    }
}

impl TouchpadPage {
    pub const fn new() -> Self {
        Self {
            host: HostState::Disconnected,
        }
    }
}

impl Page for TouchpadPage {
    type Id = Route;
    fn page_info(&self) -> PageInfo<Route> {
        PageInfo {
            presentation: Presentation::FullScreen,
            ..Route::Touchpad.info()
        }
    }
    fn enter(&mut self) {}
    fn leave(&mut self) {}
    fn handle_touch(&mut self, _: TouchEvent) -> PageEvent<Route> {
        PageEvent::None
    }
    fn render_page<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        render(display, self.host)
    }
}

pub const TOUCHPAD_TARGET: Rectangle = Rectangle::new(Point::new(28, 354), Size::new(484, 112));

pub fn render<D: DrawTarget<Color = Gray4>>(
    display: &mut D,
    state: HostState,
) -> Result<(), D::Error> {
    display.clear(STYLE.background)?;
    for (label, y) in [
        ("USB TOUCHPAD", 72),
        ("Move fingers on this surface.", 180),
        ("Linux controls taps and gestures.", 220),
        ("Press Function or Home to exit.", 300),
        (status(state), 380),
    ] {
        let role = match y {
            72 => TextRole::Title,
            _ => TextRole::Body,
        };
        Text::new(label, Point::new(48, y), STYLE.text(role)).draw(display)?;
    }
    Ok(())
}

pub fn status(state: HostState) -> &'static str {
    match state {
        HostState::Ready => "USB host ready",
        HostState::Disconnected => "Connect a USB host",
        HostState::Suspended => "USB host suspended",
        HostState::Fault => "USB input fault",
    }
}
