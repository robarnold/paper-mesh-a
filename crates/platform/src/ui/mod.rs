//! Reusable presentation components without screen destinations or fixed layouts.
//!
//! # Design choices
//!
//! - Leave placement and actions to the caller so the same control can serve
//!   different screens.
//!
//! See the [toolkit contract](../../../../docs/platform/toolkit.md#user-interface-components).

use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle};

pub mod controls;
pub mod elements;
pub mod format;
pub mod header;
pub mod layout;
pub mod navigation;
pub mod page;
pub mod shell;
pub mod style;
pub mod text;

pub use controls::{Button, EnabledState, NavigationCard};
pub use header::Header;
pub use navigation::{NavigationError, Navigator, Transition, Update};
pub use page::{
    ElementSet, Navigation, Page, PageEvent, PageInfo, PageRuntime, PageSet, Presentation,
    UiContext,
};
pub use shell::{Shell, ShellLayout};
pub use style::{Spacing, Style, TextRole};
pub use text::TextBadge;

#[cfg(test)]
mod tests;

/// This convenience renderer uses the default grayscale style.
pub fn button<D: DrawTarget<Color = Gray4>>(
    display: &mut D,
    target: Rectangle,
    label: &str,
    selected: bool,
    enabled: bool,
) -> Result<(), D::Error> {
    Button {
        bounds: target,
        label,
        selected,
        enabled,
    }
    .render(display, &Style::GRAYSCALE)
}

pub mod keyboard;
pub mod text_field;
pub use keyboard::SoftKeyboard;
pub use text_field::{TextField, TextInput};

pub mod system_keyboard;
pub use system_keyboard::{SystemKeyboard, TextFocus, TextSession};
