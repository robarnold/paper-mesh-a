//! Header rendering with caller-supplied text regions and navigation controls.

use embedded_graphics::{
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};

use super::{Button, Style, TextRole};

pub struct Header<'a> {
    pub context: &'a str,
    pub title: &'a str,
    pub context_bounds: Rectangle,
    pub title_bounds: Rectangle,
    pub divider: Rectangle,
    pub controls: &'a [Button<'a>],
}

impl Header<'_> {
    pub fn render<D: DrawTarget<Color = Gray4>>(
        &self,
        display: &mut D,
        style: &Style,
    ) -> Result<(), D::Error> {
        for (value, bounds, role) in [
            (self.context, self.context_bounds, TextRole::Caption),
            (self.title, self.title_bounds, TextRole::Title),
        ] {
            Text::with_baseline(value, bounds.top_left, style.text(role), Baseline::Top)
                .draw(&mut display.clipped(&bounds))?;
        }
        self.divider
            .into_styled(PrimitiveStyle::with_fill(style.divider))
            .draw(display)?;
        for control in self.controls {
            control.render(display, style)?;
        }
        Ok(())
    }
}
