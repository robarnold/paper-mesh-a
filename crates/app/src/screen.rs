//! Application style selection and portrait layout.

use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle};
use paper_mesh_platform::ui::{Button, NavigationCard, ShellLayout, Style};

pub const STYLE: Style = Style::GRAYSCALE;
pub const CONTENT_BOUNDS: Rectangle = Rectangle::new(Point::new(28, 104), Size::new(484, 840));
pub const BACK_TARGET: Rectangle = Rectangle::new(Point::new(408, 24), Size::new(104, 56));
const TITLE_WIDTH: u32 =
    (BACK_TARGET.top_left.x - CONTENT_BOUNDS.top_left.x) as u32 - STYLE.spacing.control_gap;
pub const LAYOUT: ShellLayout = ShellLayout {
    viewport: Rectangle::new(Point::zero(), crate::SCREEN_SIZE),
    content: CONTENT_BOUNDS,
    context: Rectangle::new(Point::new(28, 24), Size::new(TITLE_WIDTH, 12)),
    title: Rectangle::new(Point::new(28, 48), Size::new(TITLE_WIDTH, 24)),
    divider: Rectangle::new(Point::new(28, 92), Size::new(484, 1)),
    back: BACK_TARGET,
};

pub fn button<D: DrawTarget<Color = Gray4>>(
    display: &mut D,
    bounds: Rectangle,
    label: &str,
    selected: bool,
    enabled: bool,
) -> Result<(), D::Error> {
    Button {
        bounds,
        label,
        selected,
        enabled,
    }
    .render(display, &STYLE)
}

pub fn card<D: DrawTarget<Color = Gray4>>(
    display: &mut D,
    bounds: Rectangle,
    title: &str,
    description: &str,
) -> Result<(), D::Error> {
    NavigationCard {
        bounds,
        title,
        description,
        enabled: true,
    }
    .render(display, &STYLE)
}
