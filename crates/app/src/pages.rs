//! Application destinations and adapters for the platform Page contract.
//!
//! # Design choices
//!
//! - Keep power observations active across power subpages so opening the suspend
//!   experiment does not interrupt sampling.
//!
//! See the [page ownership rules](../../../docs/application-ui.md#integration-and-ownership).

use embedded_graphics::{pixelcolor::Gray4, prelude::*};
use paper_mesh_platform::ui::{Navigation, Page, PageEvent, PageInfo, PageRuntime, Presentation};

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
    Calculator,
    Diagnostics,
    Display,
    Touch,
    Radio,
    Location,
    Power,
    Suspend,
    PowerOff,
    Touchpad,
    Keyboard,
}

impl Route {
    pub const fn info(self) -> PageInfo<Self> {
        let (title, context) = match self {
            Self::Home => ("Home", "PAPER MESH"),
            Self::Calculator => ("Calculator", "PAPER MESH"),
            Self::Diagnostics => ("Diagnostics", "PAPER MESH"),
            Self::Touchpad => ("USB touchpad", "PAPER MESH"),
            Self::Display => ("Display", "DIAGNOSTICS"),
            Self::Keyboard => ("Keyboard", "DIAGNOSTICS"),
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
            Component::Keyboard => Self::Keyboard,
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
            Route::Calculator => $app.calculator.$method($($arg),*),
            Route::Diagnostics => $app.diagnostics.$method($($arg),*),
            Route::Display => $app.display_check.$method($($arg),*),
            Route::Keyboard => $app.keyboard.$method($($arg),*),
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

macro_rules! dispatch_runtime {
    ($app:ident, $route:expr, $method:ident($($arg:expr),* $(,)?)) => {
        match $route {
            Route::Home => PageRuntime::$method(&mut $app.home, $($arg),*),
            Route::Calculator => PageRuntime::$method(&mut $app.calculator, $($arg),*),
            Route::Diagnostics => PageRuntime::$method(&mut $app.diagnostics, $($arg),*),
            Route::Display => PageRuntime::$method(&mut $app.display_check, $($arg),*),
            Route::Keyboard => PageRuntime::$method(&mut $app.keyboard, $($arg),*),
            Route::Touch => PageRuntime::$method(&mut $app.touch_screen, $($arg),*),
            Route::Radio => PageRuntime::$method(&mut $app.debugger, $($arg),*),
            Route::Location => PageRuntime::$method(&mut $app.location, $($arg),*),
            Route::Power => PageRuntime::$method(&mut $app.power, $($arg),*),
            Route::Suspend => PageRuntime::$method(&mut $app.power.suspend, $($arg),*),
            Route::PowerOff => PageRuntime::$method(&mut $app.power.off, $($arg),*),
            Route::Touchpad => PageRuntime::$method(&mut $app.touchpad, $($arg),*),
        }
    };
}

pub(crate) struct Pages {
    pub calculator: crate::calculator::CalculatorPage,
    pub diagnostics: Menu,
    pub keyboard: crate::keyboard::KeyboardScreen,
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
            calculator: crate::calculator::CalculatorPage::empty(),
            diagnostics: Menu::new(),
            keyboard: crate::keyboard::KeyboardScreen::new(),
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
    fn text_focus_page(&self, id: Route) -> Option<paper_mesh_platform::ui::TextFocus> {
        dispatch_page!(self, id, text_focus())
    }
    fn text_input_page(
        &mut self,
        id: Route,
        field: u16,
        input: paper_mesh_platform::ui::TextInput,
    ) -> bool {
        dispatch_page!(self, id, text_input(field, input))
    }
    fn invalidate_page(&mut self, id: Route) {
        dispatch_runtime!(self, id, invalidate());
    }

    fn info(&self, id: Route) -> PageInfo<Route> {
        dispatch_page!(self, id, page_info())
    }

    fn enter_page(&mut self, id: Route) {
        if matches!(id, Route::Home | Route::Touchpad | Route::Calculator) {
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

    fn publish_page(
        &mut self,
        id: Route,
        tx: &mut paper_mesh_platform::composition::Transaction<'_, 64>,
        root: paper_mesh_platform::composition::LayerId,
    ) -> Result<(), paper_mesh_platform::composition::Error> {
        dispatch_page!(self, id, publish(tx, root))
    }
    fn activate_page(
        &mut self,
        id: Route,
        slot: u16,
        gesture: paper_mesh_platform::composition::Gesture,
    ) -> PageEvent<Route> {
        dispatch_runtime!(self, id, activate(slot, gesture))
    }
    fn draw_page_custom<D: DrawTarget<Color = Gray4>>(
        &self,
        id: Route,
        slot: u16,
        display: &mut D,
    ) -> Result<(), D::Error> {
        dispatch_page!(self, id, draw_custom(slot, display))
    }
}

macro_rules! simple_page {
    ($type:ty, $route:ident, $module:ident) => {
        impl Page for $type {
            type Id = Route;
            type Elements =
                paper_mesh_platform::ui::elements::Elements<Self, crate::$module::Action, 8, 0>;
            fn elements(&self) -> &Self::Elements {
                &self.elements
            }
            fn elements_mut(&mut self) -> &mut Self::Elements {
                &mut self.elements
            }
            fn sync_elements(&mut self) {
                self.sync_nodes();
            }
            fn page_info(&self) -> PageInfo<Route> {
                Route::$route.info()
            }
            fn did_enter(&mut self) {
                self.open();
            }
            fn will_leave(&mut self) {
                self.close();
            }
            fn configure_handlers(&mut self) {
                self.elements_mut()
                    .default_callback(|page, context, action| {
                        let result = PageEvent::changed(page.activate(action));
                        context.respond(result);
                    });
            }
            fn draw_custom<D: DrawTarget<Color = Gray4>>(
                &self,
                _: u16,
                display: &mut D,
            ) -> Result<(), D::Error> {
                self.render(display)
            }
        }
    };
}
simple_page!(TouchScreen, Touch, touch);
simple_page!(Debugger, Radio, debugger);

impl Page for Menu {
    type Id = Route;
    type Elements =
        paper_mesh_platform::ui::elements::Elements<Self, crate::diagnostics::Action, 8, 0>;
    fn elements(&self) -> &Self::Elements {
        &self.elements
    }
    fn elements_mut(&mut self) -> &mut Self::Elements {
        &mut self.elements
    }
    fn sync_elements(&mut self) {
        self.sync_nodes();
    }
    fn page_info(&self) -> PageInfo<Route> {
        Route::Diagnostics.info()
    }
    fn did_enter(&mut self) {
        self.open();
    }
    fn will_leave(&mut self) {
        self.close();
    }
    fn configure_handlers(&mut self) {
        self.elements_mut()
            .default_callback(|page, context, action| {
                let result = {
                    page.activate(action).map_or(PageEvent::None, |component| {
                        PageEvent::Navigate(Navigation::Open(component.into()))
                    })
                };
                context.respond(result);
            });
    }
    fn draw_custom<D: DrawTarget<Color = Gray4>>(
        &self,
        _: u16,
        display: &mut D,
    ) -> Result<(), D::Error> {
        self.render(display)
    }
}

impl Page for DisplayCheck {
    type Id = Route;
    type Elements =
        paper_mesh_platform::ui::elements::Elements<Self, crate::display_check::Action, 8, 0>;
    fn elements(&self) -> &Self::Elements {
        &self.elements
    }
    fn elements_mut(&mut self) -> &mut Self::Elements {
        &mut self.elements
    }
    fn sync_elements(&mut self) {
        self.sync_nodes();
    }
    fn page_info(&self) -> PageInfo<Route> {
        let mut info = Route::Display.info();
        if self.pattern().is_some() {
            info.presentation = Presentation::FullScreen;
        }
        info
    }
    fn did_enter(&mut self) {
        self.open();
    }
    fn will_leave(&mut self) {
        self.close();
    }
    fn configure_handlers(&mut self) {
        self.elements_mut()
            .default_callback(|page, context, action| {
                let result = PageEvent::changed(page.activate(action));
                context.respond(result);
            });
    }
    fn draw_custom<D: DrawTarget<Color = Gray4>>(
        &self,
        _: u16,
        display: &mut D,
    ) -> Result<(), D::Error> {
        self.render(display)
    }
}

impl Page for LocationScreen {
    type Id = Route;
    type Elements = paper_mesh_platform::ui::elements::Elements<Self, (), 1, 0>;
    fn elements(&self) -> &Self::Elements {
        &self.elements
    }
    fn elements_mut(&mut self) -> &mut Self::Elements {
        &mut self.elements
    }
    fn sync_elements(&mut self) {
        self.elements
            .surface([])
            .expect("fixed surface fits element capacity");
    }
    fn page_info(&self) -> PageInfo<Route> {
        Route::Location.info()
    }
    fn did_enter(&mut self) {
        self.open();
    }
    fn will_leave(&mut self) {
        self.close();
    }
    fn configure_handlers(&mut self) {
        self.elements_mut().default_callback(|_page, context, _| {
            let result = PageEvent::None;
            context.respond(result);
        });
    }
    fn draw_custom<D: DrawTarget<Color = Gray4>>(
        &self,
        _: u16,
        display: &mut D,
    ) -> Result<(), D::Error> {
        self.render(display)
    }
}

impl Page for PowerScreen {
    type Id = Route;
    type Elements = paper_mesh_platform::ui::elements::Elements<Self, crate::power::Action, 8, 0>;
    fn elements(&self) -> &Self::Elements {
        &self.elements
    }
    fn elements_mut(&mut self) -> &mut Self::Elements {
        &mut self.elements
    }
    fn sync_elements(&mut self) {
        self.sync_nodes();
    }
    fn page_info(&self) -> PageInfo<Route> {
        Route::Power.info()
    }
    fn did_enter(&mut self) {
        self.enter_page();
    }
    fn will_leave(&mut self) {
        self.leave_page();
    }
    fn configure_handlers(&mut self) {
        self.elements_mut()
            .default_callback(|page, context, action| {
                let result = page.activate(action);
                context.respond(result);
            });
    }
    fn draw_custom<D: DrawTarget<Color = Gray4>>(
        &self,
        _: u16,
        display: &mut D,
    ) -> Result<(), D::Error> {
        self.render(display)
    }
}

macro_rules! lifecycle_page {
    ($type:ty, $route:ident, $module:ident) => {
        impl Page for $type {
            type Id = Route;
            type Elements =
                paper_mesh_platform::ui::elements::Elements<Self, crate::$module::Action, 8, 0>;
            fn elements(&self) -> &Self::Elements {
                &self.elements
            }
            fn elements_mut(&mut self) -> &mut Self::Elements {
                &mut self.elements
            }
            fn sync_elements(&mut self) {
                self.sync_nodes();
            }
            fn page_info(&self) -> PageInfo<Route> {
                PageInfo {
                    navigation_enabled: !self.pending(),
                    ..Route::$route.info()
                }
            }
            fn did_enter(&mut self) {
                self.open();
            }
            fn will_leave(&mut self) {
                self.close();
            }
            fn configure_handlers(&mut self) {
                self.elements_mut()
                    .default_callback(|page, context, action| {
                        let result = PageEvent::changed(page.activate(action));
                        context.respond(result);
                    });
            }
            fn draw_custom<D: DrawTarget<Color = Gray4>>(
                &self,
                _: u16,
                display: &mut D,
            ) -> Result<(), D::Error> {
                self.render(display)
            }
        }
    };
}
lifecycle_page!(SuspendScreen, Suspend, suspend);
lifecycle_page!(PowerOffScreen, PowerOff, power_off);
