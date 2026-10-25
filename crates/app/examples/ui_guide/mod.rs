//! A small application for the UI guide. The simulator supplies its native host.

use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle, text::Text};
use paper_mesh_platform::{
    TouchEvent,
    input::TapGesture,
    ui::{
        Button, Navigation, NavigationCard, Navigator, Page, PageEvent, PageInfo, Presentation,
        Shell, ShellLayout, Style, TextRole, Transition,
    },
};

pub mod catalog;
pub mod controls;
pub mod text;

pub const SIZE: Size = Size::new(540, 960);
pub const STYLE: Style = Style::GRAYSCALE;
pub const LAYOUT: ShellLayout = ShellLayout {
    viewport: Rectangle::new(Point::zero(), SIZE),
    content: Rectangle::new(Point::new(28, 104), Size::new(484, 840)),
    context: Rectangle::new(Point::new(28, 24), Size::new(368, 12)),
    title: Rectangle::new(Point::new(28, 48), Size::new(368, 24)),
    divider: Rectangle::new(Point::new(28, 92), Size::new(484, 1)),
    back: Rectangle::new(Point::new(408, 24), Size::new(104, 56)),
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Home,
    Tools,
    Detail,
}

impl Route {
    pub fn title(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Tools => "Tools",
            Self::Detail => "Detail",
        }
    }
}

pub struct ExamplePage {
    pub route: Route,
    pub count: u32,
    pub selected: bool,
    pub full_screen: bool,
    pub pending: bool,
    pub entries: u32,
    pub exits: u32,
    gesture: TapGesture<usize>,
}

impl ExamplePage {
    fn new(route: Route) -> Self {
        Self {
            route,
            count: 0,
            selected: false,
            full_screen: false,
            pending: false,
            entries: 0,
            exits: 0,
            gesture: TapGesture::new(),
        }
    }

    pub fn card(&self) -> NavigationCard<'static> {
        NavigationCard {
            bounds: Rectangle::new(Point::new(28, 236), Size::new(484, 112)),
            title: "Open Detail",
            description: "A page with retained state",
            enabled: true,
        }
    }

    pub fn tools_card(&self) -> NavigationCard<'static> {
        NavigationCard {
            bounds: Rectangle::new(Point::new(28, 372), Size::new(484, 112)),
            title: "Open Tools",
            description: "Open Detail from another caller",
            enabled: true,
        }
    }

    pub fn buttons(&self) -> [Button<'static>; 4] {
        let labels = [
            ("Count a tap", false),
            (
                if self.selected {
                    "Choice: selected"
                } else {
                    "Choice: unselected"
                },
                self.selected,
            ),
            (
                if self.full_screen {
                    "Exit full screen"
                } else {
                    "Enter full screen"
                },
                false,
            ),
            (
                if self.pending {
                    "Operation pending"
                } else {
                    "Start example operation"
                },
                false,
            ),
        ];
        core::array::from_fn(|index| Button {
            bounds: Rectangle::new(Point::new(28, 236 + index as i32 * 84), Size::new(484, 56)),
            label: labels[index].0,
            selected: labels[index].1,
            enabled: !self.pending,
        })
    }
}

impl Page for ExamplePage {
    type Id = Route;

    // guide:page:start
    fn page_info(&self) -> PageInfo<Route> {
        PageInfo {
            id: self.route,
            title: self.route.title(),
            context: "EXAMPLE APPLICATION",
            presentation: if self.full_screen {
                Presentation::FullScreen
            } else {
                Presentation::Standard
            },
            navigation_enabled: !self.pending,
        }
    }
    // guide:page:end

    // guide:lifecycle:start
    fn enter(&mut self) {
        self.gesture.cancel();
        self.entries += 1;
    }

    fn leave(&mut self) {
        self.gesture.cancel();
        self.full_screen = false;
        self.exits += 1;
        // The count and selection remain in this page object.
    }
    // guide:lifecycle:end

    fn handle_touch(&mut self, event: TouchEvent) -> PageEvent<Route> {
        let card = self.card();
        let tools_card = self.tools_card();
        let buttons = self.buttons();
        let home = self.route == Route::Home;
        let menu = self.route != Route::Detail;
        let action = self.gesture.update(event, |point| {
            if menu {
                if card.hit(point) {
                    Some(0)
                } else if home && tools_card.hit(point) {
                    Some(1)
                } else {
                    None
                }
            } else {
                buttons.iter().position(|button| button.hit(point))
            }
        });
        match action {
            Some(0) if menu => PageEvent::Navigate(Navigation::Open(Route::Detail)),
            Some(1) if menu => PageEvent::Navigate(Navigation::Open(Route::Tools)),
            Some(0) => {
                self.count += 1;
                PageEvent::Changed
            }
            Some(1) => {
                self.selected = !self.selected;
                PageEvent::Changed
            }
            Some(2) => {
                self.full_screen = !self.full_screen;
                PageEvent::Changed
            }
            Some(3) => {
                self.pending = true;
                PageEvent::Changed
            }
            _ => PageEvent::None,
        }
    }

    fn render_page<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        if self.full_screen {
            line(display, "Full-screen Detail", 48, TextRole::Title)?;
            line(display, "The shell hides its header.", 84, TextRole::Body)?;
        }
        if self.route != Route::Detail {
            line(display, "One shell. Three pages.", 144, TextRole::Title)?;
            line(
                display,
                "Open Detail to try the controls.",
                184,
                TextRole::Body,
            )?;
            self.card().render(display, &STYLE)?;
            if self.route == Route::Home {
                self.tools_card().render(display, &STYLE)?;
            }
            line(
                display,
                "Back returns to the calling page.",
                540,
                TextRole::Body,
            )?;
            line(
                display,
                "Detail keeps the same page state.",
                576,
                TextRole::Body,
            )?;
        } else {
            line(
                display,
                "This page owns its content and state.",
                144,
                TextRole::Body,
            )?;
            line(
                display,
                &paper_mesh_platform::ui::format::measurement(
                    Some(self.count),
                    if self.count == 1 { "tap" } else { "taps" },
                ),
                184,
                TextRole::Body,
            )?;
            for button in self.buttons() {
                button.render(display, &STYLE)?;
            }
            if self.pending {
                line(
                    display,
                    "The example operation is pending.",
                    620,
                    TextRole::Body,
                )?;
                line(
                    display,
                    "Select Finish operation beside the preview.",
                    660,
                    TextRole::Body,
                )?;
            } else {
                line(
                    display,
                    "Back returns to the calling page.",
                    620,
                    TextRole::Body,
                )?;
                line(
                    display,
                    "The hardware Home key returns Home.",
                    660,
                    TextRole::Body,
                )?;
            }
            line(
                display,
                "Count and choice survive a page change.",
                760,
                TextRole::Secondary,
            )?;
        }
        Ok(())
    }
}

pub fn line<D: DrawTarget<Color = Gray4>>(
    display: &mut D,
    text: &str,
    y: i32,
    role: TextRole,
) -> Result<(), D::Error> {
    Text::new(text, Point::new(28, y), STYLE.text(role)).draw(display)?;
    Ok(())
}

pub struct ExamplePages {
    pub home: ExamplePage,
    pub tools: ExamplePage,
    pub detail: ExamplePage,
}

impl ExamplePages {
    pub fn page(&self, route: Route) -> &ExamplePage {
        match route {
            Route::Home => &self.home,
            Route::Tools => &self.tools,
            Route::Detail => &self.detail,
        }
    }

    fn page_mut(&mut self, route: Route) -> &mut ExamplePage {
        match route {
            Route::Home => &mut self.home,
            Route::Tools => &mut self.tools,
            Route::Detail => &mut self.detail,
        }
    }
}

impl paper_mesh_platform::ui::PageSet for ExamplePages {
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

// guide:composition:start
pub struct Example {
    pub ui: Navigator<ExamplePages, 8>,
    pub transition: Option<Transition<Route>>,
}

impl Default for Example {
    fn default() -> Self {
        let pages = ExamplePages {
            home: ExamplePage::new(Route::Home),
            tools: ExamplePage::new(Route::Tools),
            detail: ExamplePage::new(Route::Detail),
        };
        Self {
            ui: Navigator::launch(Route::Home, Shell::new(LAYOUT), pages),
            transition: None,
        }
    }
}
// guide:composition:end

impl Example {
    pub fn page(&self) -> &ExamplePage {
        self.ui.pages().page(self.ui.current())
    }

    // guide:navigation:start
    pub fn navigate(&mut self, action: Navigation<Route>) {
        if let Ok(Some(transition)) = self.ui.navigate(action) {
            self.transition = Some(transition);
        }
    }
    // guide:navigation:end

    // guide:input:start
    pub fn touch(&mut self, event: TouchEvent) {
        if let Ok(update) = self.ui.touch(event)
            && let Some(transition) = update.transition
        {
            self.transition = Some(transition);
        }
    }
    // guide:input:end

    // guide:render:start
    pub fn render<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        self.ui.render(display, &STYLE)
    }
    // guide:render:end
}
