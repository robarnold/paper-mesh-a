//! Home destinations and retained uptime presentation.
//!
//! # Design choices
//!
//! - Clear the pending tap on entry and exit so a press from a previous visit cannot
//!   open a destination on release.
//!
//! See the [application navigation rules](../../../docs/application-ui.md#destinations-and-operation-restrictions).

use core::fmt::Write;
use embedded_graphics::{pixelcolor::Gray4, prelude::*, text::Text};
use heapless::String;
use paper_mesh_platform::ui::EnabledState;
use paper_mesh_platform::ui::{Navigation, Page, PageEvent, PageInfo, TextRole};

use crate::{
    diagnostics,
    pages::Route,
    screen::{self, STYLE},
    touchpad,
};

pub struct HomePage {
    pub uptime_seconds: u64,
    elements: paper_mesh_platform::ui::elements::Elements<HomePage, Route, 4, 0>,
}

impl HomePage {
    pub const fn new() -> Self {
        Self {
            uptime_seconds: 0,
            elements: paper_mesh_platform::ui::elements::Elements::new(screen::LAYOUT.viewport),
        }
    }
}

impl Page for HomePage {
    type Id = Route;
    fn page_info(&self) -> PageInfo<Route> {
        Route::Home.info()
    }
    type Elements = paper_mesh_platform::ui::elements::Elements<Self, Route, 4, 0>;
    fn elements(&self) -> &Self::Elements {
        &self.elements
    }
    fn elements_mut(&mut self) -> &mut Self::Elements {
        &mut self.elements
    }
    fn sync_elements(&mut self) {
        self.elements
            .surface([
                (
                    diagnostics::HOME_TARGET,
                    Route::Diagnostics,
                    EnabledState::Enabled,
                ),
                (
                    touchpad::TOUCHPAD_TARGET,
                    Route::Touchpad,
                    EnabledState::Enabled,
                ),
                (
                    crate::calculator::HOME_TARGET,
                    Route::Calculator,
                    EnabledState::Enabled,
                ),
            ])
            .expect("fixed surface fits element capacity");
    }
    fn configure_handlers(&mut self) {
        self.elements_mut()
            .default_callback(|_page, context, route| {
                let result = PageEvent::Navigate(Navigation::Open(route));
                context.respond(result);
            });
    }
    fn draw_custom<D: DrawTarget<Color = Gray4>>(
        &self,
        _: u16,
        display: &mut D,
    ) -> Result<(), D::Error> {
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
        screen::card(
            display,
            crate::calculator::HOME_TARGET,
            "Calculator",
            "Basic decimal arithmetic",
        )?;
        let _ = write!(label, "Uptime: {} s", self.uptime_seconds);
        Text::new(&label, Point::new(28, 900), STYLE.text(TextRole::Secondary)).draw(display)?;
        Ok(())
    }
}
