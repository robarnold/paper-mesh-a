//! Home destinations and retained uptime presentation.

use core::fmt::Write;
use embedded_graphics::{pixelcolor::Gray4, prelude::*, text::Text};
use heapless::String;
use paper_mesh_platform::{
    TouchEvent,
    input::TapGesture,
    ui::{Navigation, Page, PageEvent, PageInfo, TextRole},
};

use crate::{
    diagnostics,
    pages::Route,
    screen::{self, STYLE},
    touchpad,
};

pub struct HomePage {
    pub uptime_seconds: u64,
    gesture: TapGesture<Route>,
}

impl HomePage {
    pub const fn new() -> Self {
        Self {
            uptime_seconds: 0,
            gesture: TapGesture::new(),
        }
    }
}

impl Page for HomePage {
    type Id = Route;
    fn page_info(&self) -> PageInfo<Route> {
        Route::Home.info()
    }
    fn enter(&mut self) {
        self.gesture.cancel();
    }
    fn leave(&mut self) {
        self.gesture.cancel();
    }
    fn handle_touch(&mut self, event: TouchEvent) -> PageEvent<Route> {
        self.gesture
            .update(event, |point| {
                [
                    (diagnostics::HOME_TARGET, Route::Diagnostics),
                    (touchpad::TOUCHPAD_TARGET, Route::Touchpad),
                ]
                .into_iter()
                .find_map(|(bounds, route)| bounds.contains(point).then_some(route))
            })
            .map_or(PageEvent::None, |route| {
                PageEvent::Navigate(Navigation::Open(route))
            })
    }
    fn render_page<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        Text::new(
            "Device tools",
            Point::new(28, 132),
            STYLE.text(TextRole::Body),
        )
        .draw(display)?;
        screen::card(
            display,
            diagnostics::HOME_TARGET,
            "Diagnostics",
            "Inspect and test device components",
        )?;
        screen::card(
            display,
            touchpad::TOUCHPAD_TARGET,
            "USB touchpad",
            "Use the device as a USB touchpad",
        )?;
        let mut label = String::<64>::new();
        let _ = write!(label, "Uptime: {} s", self.uptime_seconds);
        Text::new(&label, Point::new(28, 900), STYLE.text(TextRole::Secondary)).draw(display)?;
        Ok(())
    }
}
