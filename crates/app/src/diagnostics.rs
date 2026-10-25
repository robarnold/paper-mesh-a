//! Component selection and common layout for application diagnostics.

use crate::screen::STYLE;
use paper_mesh_platform::ui::TextRole;

use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle, text::Text};
use paper_mesh_platform::{TouchEvent, input::TapGesture, ui::NavigationCard};

pub const HOME_TARGET: Rectangle = Rectangle::new(Point::new(28, 206), Size::new(484, 112));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component {
    Display,
    Touch,
    Radio,
    Location,
    Power,
}

impl Component {
    pub const ALL: [Self; 5] = [
        Self::Display,
        Self::Touch,
        Self::Radio,
        Self::Location,
        Self::Power,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Display => "Display",
            Self::Touch => "Touch",
            Self::Radio => "Radio",
            Self::Location => "GNSS",
            Self::Power => "Power",
        }
    }

    pub const fn target(self) -> Rectangle {
        Rectangle::new(Point::new(28, 170 + self as i32 * 140), Size::new(484, 112))
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Display => "Grayscale and full refresh checks",
            Self::Touch => "Tap response and gesture checks",
            Self::Radio => "Wi-Fi and Bluetooth LE scans",
            Self::Location => "Position, receiver, and satellites",
            Self::Power => "Battery, charger, and power controls",
        }
    }
}

#[derive(PartialEq)]
enum Action {
    Open(Component),
}

pub struct Menu {
    open: bool,
    gesture: TapGesture<Action>,
}

impl Default for Menu {
    fn default() -> Self {
        Self::new()
    }
}

impl Menu {
    pub const fn new() -> Self {
        Self {
            open: false,
            gesture: TapGesture::new(),
        }
    }

    pub fn open(&mut self) {
        self.open = true;
        self.gesture.cancel();
    }

    pub fn close(&mut self) {
        self.open = false;
        self.gesture.cancel();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn touch(&mut self, event: TouchEvent) -> Option<Component> {
        self.gesture
            .update(event, |point| {
                Component::ALL
                    .into_iter()
                    .find(|component| {
                        NavigationCard {
                            bounds: component.target(),
                            title: component.label(),
                            description: component.description(),
                            enabled: true,
                        }
                        .hit(point)
                    })
                    .map(Action::Open)
            })
            .map(|Action::Open(component)| component)
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        display.clear(STYLE.background)?;
        Text::new(
            "Select a component.",
            Point::new(28, 132),
            STYLE.text(TextRole::Body),
        )
        .draw(display)?;
        for component in Component::ALL {
            crate::screen::card(
                display,
                component.target(),
                component.label(),
                component.description(),
            )?;
        }
        Ok(())
    }
}
