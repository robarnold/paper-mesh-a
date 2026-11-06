//! Component selection and common layout for application diagnostics.
//!
//! # Design choices
//!
//! - Use each component’s card bounds for both drawing and hit testing so moving a
//!   card also moves its touch target.
//!
//! See the [diagnostics overview](../../../docs/application.md#diagnostics).

use crate::screen::STYLE;
use paper_mesh_platform::ui::EnabledState;
use paper_mesh_platform::ui::TextRole;

use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle, text::Text};

pub const HOME_TARGET: Rectangle = Rectangle::new(Point::new(28, 206), Size::new(484, 112));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component {
    Display,
    Touch,
    Radio,
    Location,
    Power,
    Keyboard,
}

impl Component {
    pub const ALL: [Self; 6] = [
        Self::Display,
        Self::Touch,
        Self::Radio,
        Self::Location,
        Self::Power,
        Self::Keyboard,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Keyboard => "Keyboard",
            Self::Display => "Display",
            Self::Touch => "Touch",
            Self::Radio => "Radio",
            Self::Location => "GNSS",
            Self::Power => "Power",
        }
    }

    pub const fn target(self) -> Rectangle {
        Rectangle::new(Point::new(28, 170 + self as i32 * 128), Size::new(484, 112))
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::Keyboard => "Type and erase text",
            Self::Display => "Grayscale and full refresh checks",
            Self::Touch => "Tap response and gesture checks",
            Self::Radio => "Wi-Fi and Bluetooth LE scans",
            Self::Location => "Position, receiver, and satellites",
            Self::Power => "Battery, charger, and power controls",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Open(Component),
}

pub struct Menu {
    open: bool,
    pub(crate) elements: paper_mesh_platform::ui::elements::Elements<Menu, Action, 8, 0>,
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
            elements: paper_mesh_platform::ui::elements::Elements::new(
                crate::screen::LAYOUT.viewport,
            ),
        }
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub(crate) fn sync_nodes(&mut self) {
        self.elements
            .surface(
                Component::ALL
                    .into_iter()
                    .map(|c| (c.target(), Action::Open(c), EnabledState::Enabled)),
            )
            .expect("fixed surface fits element capacity");
    }
    pub(crate) fn activate(&mut self, action: Action) -> Option<Component> {
        let Action::Open(component) = action;
        Some(component)
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
