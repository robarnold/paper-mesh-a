//! Interactive control examples with their own navigation and retained state.
//!
//! # Design choices
//!
//! - The navigation example keeps its own page state so a preview action can
//!   demonstrate a caller return in isolation.
//!
//! See [Pages and shells](../../../../docs/platform/toolkit.md#pages-and-shells).

use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle, text::Text};
use paper_mesh_platform::ui::EnabledState;
use paper_mesh_platform::ui::{
    Button, Navigation, NavigationCard, Navigator, Page, PageEvent, PageInfo, PageRuntime,
    Presentation, Shell, TextRole, format::measurement,
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
    pub pan_distance: i32,
    pub selected: bool,
    pub pending: bool,
    elements: paper_mesh_platform::ui::elements::Elements<ControlsPage, usize, 7, 0>,
}

impl ControlsPage {
    fn new(route: Route) -> Self {
        Self {
            route,
            count: 0,
            pan_distance: 0,
            selected: true,
            pending: false,
            elements: paper_mesh_platform::ui::elements::Elements::new(super::LAYOUT.viewport),
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

    fn did_enter(&mut self) {}
    fn will_leave(&mut self) {}

    type Elements = paper_mesh_platform::ui::elements::Elements<Self, usize, 7, 0>;
    fn elements(&self) -> &Self::Elements {
        &self.elements
    }
    fn elements_mut(&mut self) -> &mut Self::Elements {
        &mut self.elements
    }
    fn sync_elements(&mut self) {
        let buttons = self.buttons();
        let card = self.card();
        let finish = self.finish_button();
        let enabled = self.route == Route::Controls;
        self.elements
            .surface(
                buttons
                    .into_iter()
                    .enumerate()
                    .map(|(i, b)| (b.bounds, i, EnabledState::from(enabled && b.enabled)))
                    .chain([
                        (card.bounds, 4, EnabledState::from(enabled && card.enabled)),
                        (
                            finish.bounds,
                            5,
                            EnabledState::from(enabled && finish.enabled),
                        ),
                    ]),
            )
            .expect("fixed surface fits element capacity");
        let root = self.elements.root_element().unwrap();
        self.elements
            .did_pan(root, 0, |page, context, _, gesture| {
                if let paper_mesh_platform::composition::Gesture::Pan { displacement, .. } = gesture
                {
                    page.pan_distance = displacement.x;
                    context.changed();
                }
            })
            .unwrap();
    }
    fn configure_handlers(&mut self) {
        self.elements_mut()
            .default_callback(|page, context, action| {
                let result = (|| {
                    match Some(action) {
                        Some(0) => page.count = page.count.saturating_add(1),
                        Some(1) => page.selected = !page.selected,
                        Some(3) => page.pending = true,
                        Some(4) => return PageEvent::Navigate(Navigation::Open(Route::Detail)),
                        Some(5) => page.pending = false,
                        _ => return PageEvent::None,
                    }
                    PageEvent::Changed
                })();
                context.respond(result);
            });
    }

    fn draw_custom<D: DrawTarget<Color = Gray4>>(
        &self,
        _: u16,
        display: &mut D,
    ) -> Result<(), D::Error> {
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
        line(
            display,
            "Drag a button to pan; release without dragging to tap.",
            820,
            TextRole::Secondary,
        )?;
        let distance = measurement(Some(self.pan_distance), "px pan");
        line(display, &distance, 856, TextRole::Body)?;
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
    fn invalidate_page(&mut self, id: Route) {
        self.page_mut(id).invalidate();
    }
    fn info(&self, id: Route) -> PageInfo<Route> {
        self.page(id).page_info()
    }
    fn enter_page(&mut self, id: Route) {
        self.page_mut(id).enter();
    }
    fn leave_page(&mut self, id: Route) {
        self.page_mut(id).leave();
    }
    fn publish_page(
        &mut self,
        id: Route,
        tx: &mut paper_mesh_platform::composition::Transaction<'_, 64>,
        root: paper_mesh_platform::composition::LayerId,
    ) -> Result<(), paper_mesh_platform::composition::Error> {
        self.page_mut(id).publish(tx, root)
    }
    fn activate_page(
        &mut self,
        id: Route,
        slot: u16,
        gesture: paper_mesh_platform::composition::Gesture,
    ) -> PageEvent<Route> {
        self.page_mut(id).activate(slot, gesture)
    }
    fn draw_page_custom<D: DrawTarget<Color = Gray4>>(
        &self,
        id: Route,
        slot: u16,
        display: &mut D,
    ) -> Result<(), D::Error> {
        self.page(id).draw_custom(slot, display)
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
