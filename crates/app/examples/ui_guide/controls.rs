//! Interactive control examples with their own navigation and retained state.

use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle, text::Text};
use paper_mesh_platform::{
    TouchEvent,
    input::TapGesture,
    ui::{
        Button, Navigation, NavigationCard, Navigator, Page, PageEvent, PageInfo, Presentation,
        Shell, TextRole, format::measurement,
    },
};

use super::{LAYOUT, STYLE, line};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Controls,
    Detail,
}

impl Route {
    pub fn title(self) -> &'static str {
        match self {
            Self::Controls => "Controls",
            Self::Detail => "Component details",
        }
    }
}

pub struct ControlsPage {
    route: Route,
    pub count: u32,
    pub selected: bool,
    pub pending: bool,
    gesture: TapGesture<usize>,
}

impl ControlsPage {
    fn new(route: Route) -> Self {
        Self {
            route,
            count: 0,
            selected: true,
            pending: false,
            gesture: TapGesture::new(),
        }
    }

    // guide:button:start
    pub fn buttons(&self) -> [Button<'static>; 4] {
        let states = [
            ("Action", false, !self.pending),
            (
                if self.selected {
                    "Selected"
                } else {
                    "Unselected"
                },
                self.selected,
                !self.pending,
            ),
            ("Unavailable", false, false),
            (
                if self.pending {
                    "Pending"
                } else {
                    "Start operation"
                },
                false,
                !self.pending,
            ),
        ];
        core::array::from_fn(|index| Button {
            bounds: Rectangle::new(
                Point::new(28 + (index % 2) as i32 * 248, 152 + (index / 2) as i32 * 72),
                Size::new(236, 56),
            ),
            label: states[index].0,
            selected: states[index].1,
            enabled: states[index].2,
        })
    }
    // guide:button:end

    // guide:card:start
    pub fn card(&self) -> NavigationCard<'static> {
        NavigationCard {
            bounds: Rectangle::new(Point::new(28, 350), Size::new(484, 112)),
            title: "Component details",
            description: "Open this page, then use Back",
            enabled: !self.pending,
        }
    }
    // guide:card:end

    pub fn finish_button(&self) -> Button<'static> {
        Button {
            bounds: Rectangle::new(Point::new(28, 576), Size::new(484, 56)),
            label: "Finish operation",
            selected: false,
            enabled: self.pending,
        }
    }
}

impl Page for ControlsPage {
    type Id = Route;

    fn page_info(&self) -> PageInfo<Route> {
        PageInfo {
            id: self.route,
            title: self.route.title(),
            context: "PLATFORM UI",
            presentation: Presentation::Standard,
            navigation_enabled: !self.pending,
        }
    }

    fn enter(&mut self) {
        self.gesture.cancel();
    }
    fn leave(&mut self) {
        self.gesture.cancel();
    }

    fn handle_touch(&mut self, event: TouchEvent) -> PageEvent<Route> {
        if self.route != Route::Controls {
            return PageEvent::None;
        }
        let buttons = self.buttons();
        let card = self.card();
        let finish = self.finish_button();
        let action = self.gesture.update(event, |point| {
            buttons
                .iter()
                .position(|button| button.hit(point))
                .or_else(|| card.hit(point).then_some(4))
                .or_else(|| finish.hit(point).then_some(5))
        });
        match action {
            Some(0) => self.count = self.count.saturating_add(1),
            Some(1) => self.selected = !self.selected,
            Some(3) => self.pending = true,
            Some(4) => return PageEvent::Navigate(Navigation::Open(Route::Detail)),
            Some(5) => self.pending = false,
            _ => return PageEvent::None,
        }
        PageEvent::Changed
    }

    fn render_page<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        if self.route == Route::Detail {
            line(display, "The card opened this page.", 144, TextRole::Body)?;
            line(
                display,
                "Select Back to return to Controls.",
                184,
                TextRole::Body,
            )?;
            return line(
                display,
                "The guide stays on the Controls tab.",
                224,
                TextRole::Body,
            );
        }
        line(display, "CONTROL STATES", 130, TextRole::Caption)?;
        for button in self.buttons() {
            button.render(display, &STYLE)?;
        }
        line(display, "DESTINATION CARD", 322, TextRole::Caption)?;
        self.card().render(display, &STYLE)?;
        line(display, "Action count", 514, TextRole::Body)?;
        Text::new(
            &measurement(Some(self.count), ""),
            Point::new(280, 514),
            STYLE.text(TextRole::Body),
        )
        .draw(display)?;
        line(
            display,
            if self.pending {
                "Operation pending. Select Finish operation."
            } else {
                "Select Start operation to try Pending."
            },
            550,
            TextRole::Body,
        )?;
        self.finish_button().render(display, &STYLE)?;
        line(
            display,
            "Tap Selected to change the selection.",
            680,
            TextRole::Body,
        )?;
        line(
            display,
            "Drag outside to cancel a tap.",
            716,
            TextRole::Body,
        )?;
        line(
            display,
            "Unavailable stays disabled in this example.",
            752,
            TextRole::Secondary,
        )
    }
}

pub struct ControlsPages {
    pub controls: ControlsPage,
    detail: ControlsPage,
}

impl ControlsPages {
    fn page(&self, route: Route) -> &ControlsPage {
        match route {
            Route::Controls => &self.controls,
            Route::Detail => &self.detail,
        }
    }
    fn page_mut(&mut self, route: Route) -> &mut ControlsPage {
        match route {
            Route::Controls => &mut self.controls,
            Route::Detail => &mut self.detail,
        }
    }
}

impl paper_mesh_platform::ui::PageSet for ControlsPages {
    type Id = Route;
    fn info(&self, id: Route) -> PageInfo<Route> {
        self.page(id).page_info()
    }
    fn enter_page(&mut self, id: Route) {
        self.page_mut(id).enter();
    }
    fn leave_page(&mut self, id: Route) {
        self.page_mut(id).leave();
    }
    fn touch_page(&mut self, id: Route, event: TouchEvent) -> PageEvent<Route> {
        self.page_mut(id).handle_touch(event)
    }
    fn draw_page<D: DrawTarget<Color = Gray4>>(
        &self,
        id: Route,
        display: &mut D,
    ) -> Result<(), D::Error> {
        self.page(id).render_page(display)
    }
}

pub struct ControlsExample {
    pub ui: Navigator<ControlsPages, 2>,
}

impl Default for ControlsExample {
    fn default() -> Self {
        Self {
            ui: Navigator::launch(
                Route::Controls,
                Shell::new(LAYOUT),
                ControlsPages {
                    controls: ControlsPage::new(Route::Controls),
                    detail: ControlsPage::new(Route::Detail),
                },
            ),
        }
    }
}
