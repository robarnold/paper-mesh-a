//! Common controls and value formatting for application screens.

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};

pub mod format;

/// This function draws a button with the supplied bounds, label, and state.
pub fn button<D: DrawTarget<Color = Gray4>>(
    display: &mut D,
    target: Rectangle,
    label: &str,
    selected: bool,
    enabled: bool,
) -> Result<(), D::Error> {
    let fill = if selected && enabled {
        Gray4::BLACK
    } else {
        Gray4::new(14)
    };
    let text = if !enabled {
        Gray4::new(8)
    } else if selected {
        Gray4::WHITE
    } else {
        Gray4::BLACK
    };
    target
        .into_styled(PrimitiveStyle::with_fill(fill))
        .draw(display)?;
    Text::new(
        label,
        Point::new(
            target.top_left.x + 16,
            target.top_left.y + target.size.height as i32 / 2 + 7,
        ),
        MonoTextStyle::new(&FONT_10X20, text),
    )
    .draw(display)?;
    Ok(())
}
