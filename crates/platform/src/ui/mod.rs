//! Reusable presentation components without screen destinations or fixed layouts.

use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle};

pub mod controls;
pub mod format;
pub mod header;
pub mod navigation;
pub mod page;
pub mod shell;
pub mod style;
pub mod text;

pub use controls::{Button, NavigationCard};
pub use header::Header;
pub use navigation::{NavigationError, Navigator, Transition, Update};
pub use page::{Navigation, Page, PageEvent, PageInfo, PageSet, Presentation};
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
