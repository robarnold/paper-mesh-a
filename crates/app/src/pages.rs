//! Application destinations and adapters for the platform Page contract.

use embedded_graphics::{pixelcolor::Gray4, prelude::*};
use paper_mesh_platform::{
    TouchEvent,
    ui::{Navigation, Page, PageEvent, PageInfo, Presentation},
};

use crate::{
    debugger::Debugger,
    diagnostics::{Component, Menu},
    display_check::DisplayCheck,
    location::LocationScreen,
    power::PowerScreen,
    power_off::PowerOffScreen,
    suspend::SuspendScreen,
    touch::TouchScreen,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Home,
    Diagnostics,
    Display,
    Touch,
    Radio,
    Location,
    Power,
    Suspend,
    PowerOff,
    Touchpad,
}

impl Route {
    pub const fn info(self) -> PageInfo<Self> {
        let (title, context) = match self {
            Self::Home => ("Home", "PAPER MESH"),
            Self::Diagnostics => ("Diagnostics", "PAPER MESH"),
            Self::Touchpad => ("USB touchpad", "PAPER MESH"),
            Self::Display => ("Display", "DIAGNOSTICS"),
            Self::Touch => ("Touch", "DIAGNOSTICS"),
            Self::Radio => ("Radio", "DIAGNOSTICS"),
            Self::Location => ("GNSS", "DIAGNOSTICS"),
            Self::Power => ("Power", "DIAGNOSTICS"),
            Self::Suspend => ("Power / Suspend", "DIAGNOSTICS"),
            Self::PowerOff => ("Power / Power off", "DIAGNOSTICS"),
        };
        PageInfo {
            id: self,
            title,
            context,
            presentation: Presentation::Standard,
            navigation_enabled: true,
        }
    }

    pub const fn is_power(self) -> bool {
        matches!(self, Self::Power | Self::Suspend | Self::PowerOff)
    }
}

impl From<Component> for Route {
    fn from(component: Component) -> Self {
        match component {
            Component::Display => Self::Display,
            Component::Touch => Self::Touch,
            Component::Radio => Self::Radio,
            Component::Location => Self::Location,
            Component::Power => Self::Power,
        }
    }
}

// Dispatch uses concrete page storage without a heap or trait objects.
macro_rules! dispatch_page {
    ($app:ident, $route:expr, $method:ident($($arg:expr),* $(,)?)) => {
        match $route {
            Route::Home => $app.home.$method($($arg),*),
            Route::Diagnostics => $app.diagnostics.$method($($arg),*),
            Route::Display => $app.display_check.$method($($arg),*),
            Route::Touch => $app.touch_screen.$method($($arg),*),
            Route::Radio => $app.debugger.$method($($arg),*),
            Route::Location => $app.location.$method($($arg),*),
            Route::Power => $app.power.$method($($arg),*),
            Route::Suspend => $app.power.suspend.$method($($arg),*),
            Route::PowerOff => $app.power.off.$method($($arg),*),
            Route::Touchpad => $app.touchpad.$method($($arg),*),
        }
    };
}

pub(crate) struct Pages {
    pub diagnostics: Menu,
    pub display_check: DisplayCheck,
    pub touch_screen: TouchScreen,
    pub touchpad: crate::touchpad::TouchpadPage,
    pub home: crate::home::HomePage,
    pub debugger: Debugger,
    pub location: LocationScreen,
    pub power: PowerScreen,
}

impl Pages {
    pub const fn new() -> Self {
        Self {
            diagnostics: Menu::new(),
            display_check: DisplayCheck::new(),
            touch_screen: TouchScreen::new(),
            touchpad: crate::touchpad::TouchpadPage::new(),
            home: crate::home::HomePage::new(),
            debugger: Debugger::new(),
            location: LocationScreen::new(),
            power: PowerScreen::new(),
        }
    }
}

impl paper_mesh_platform::ui::PageSet for Pages {
    type Id = Route;

    fn info(&self, id: Route) -> PageInfo<Route> {
        dispatch_page!(self, id, page_info())
    }

    fn enter_page(&mut self, id: Route) {
        if matches!(id, Route::Home | Route::Touchpad) {
            self.diagnostics.close();
        } else {
            self.diagnostics.open();
        }
        if !id.is_power() {
            self.power.close();
        } else if !self.power.is_open() {
            self.power.open();
        }
        dispatch_page!(self, id, enter());
    }

    fn leave_page(&mut self, id: Route) {
        dispatch_page!(self, id, leave());
    }

    fn touch_page(&mut self, id: Route, event: TouchEvent) -> PageEvent<Route> {
        dispatch_page!(self, id, handle_touch(event))
    }

    fn draw_page<D: DrawTarget<Color = Gray4>>(
        &self,
        id: Route,
        display: &mut D,
    ) -> Result<(), D::Error> {
        dispatch_page!(self, id, render_page(display))
    }
}

macro_rules! simple_page {
    ($type:ty, $route:ident) => {
        impl Page for $type {
            type Id = Route;
            fn page_info(&self) -> PageInfo<Route> {
                Route::$route.info()
            }
            fn enter(&mut self) {
                self.open();
            }
            fn leave(&mut self) {
                self.close();
            }
            fn handle_touch(&mut self, event: TouchEvent) -> PageEvent<Route> {
                PageEvent::changed(self.touch(event))
            }
            fn render_page<D: DrawTarget<Color = Gray4>>(
                &self,
                display: &mut D,
            ) -> Result<(), D::Error> {
                self.render(display)
            }
        }
    };
}
simple_page!(TouchScreen, Touch);
simple_page!(Debugger, Radio);

impl Page for Menu {
    type Id = Route;
    fn page_info(&self) -> PageInfo<Route> {
        Route::Diagnostics.info()
    }
    fn enter(&mut self) {
        self.open();
    }
    fn leave(&mut self) {
        self.close();
    }
    fn handle_touch(&mut self, event: TouchEvent) -> PageEvent<Route> {
        self.touch(event).map_or(PageEvent::None, |component| {
            PageEvent::Navigate(Navigation::Open(component.into()))
        })
    }
    fn render_page<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        self.render(display)
    }
}

impl Page for DisplayCheck {
    type Id = Route;
    fn page_info(&self) -> PageInfo<Route> {
        let mut info = Route::Display.info();
        if self.pattern().is_some() {
            info.presentation = Presentation::FullScreen;
        }
        info
    }
    fn enter(&mut self) {
        self.open();
    }
    fn leave(&mut self) {
        self.close();
    }
    fn handle_touch(&mut self, event: TouchEvent) -> PageEvent<Route> {
        PageEvent::changed(self.touch(event))
    }
    fn render_page<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        self.render(display)
    }
}

impl Page for LocationScreen {
    type Id = Route;
    fn page_info(&self) -> PageInfo<Route> {
        Route::Location.info()
    }
    fn enter(&mut self) {
        self.open();
    }
    fn leave(&mut self) {
        self.close();
    }
    fn handle_touch(&mut self, _: TouchEvent) -> PageEvent<Route> {
        PageEvent::None
    }
    fn render_page<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        self.render(display)
    }
}

impl Page for PowerScreen {
    type Id = Route;
    fn page_info(&self) -> PageInfo<Route> {
        Route::Power.info()
    }
    fn enter(&mut self) {
        self.enter_page();
    }
    fn leave(&mut self) {
        self.leave_page();
    }
    fn handle_touch(&mut self, event: TouchEvent) -> PageEvent<Route> {
        self.touch(event)
    }
    fn render_page<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        self.render(display)
    }
}

macro_rules! lifecycle_page {
    ($type:ty, $route:ident) => {
        impl Page for $type {
            type Id = Route;
            fn page_info(&self) -> PageInfo<Route> {
                PageInfo {
                    navigation_enabled: !self.pending(),
                    ..Route::$route.info()
                }
            }
            fn enter(&mut self) {
                self.open();
            }
            fn leave(&mut self) {
                self.close();
            }
            fn handle_touch(&mut self, event: TouchEvent) -> PageEvent<Route> {
                PageEvent::changed(self.touch(event))
            }
            fn render_page<D: DrawTarget<Color = Gray4>>(
                &self,
                display: &mut D,
            ) -> Result<(), D::Error> {
                self.render(display)
            }
        }
    };
}
lifecycle_page!(SuspendScreen, Suspend);
lifecycle_page!(PowerOffScreen, PowerOff);
