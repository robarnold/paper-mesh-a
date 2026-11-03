//! Bounded single-line text field.

use super::{Style, TextRole};
use embedded_graphics::{
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyleBuilder, Rectangle},
    text::{Baseline, Text},
};
use heapless::String;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextInput {
    Character(char),
    Backspace,
}

#[derive(Default)]
pub struct TextField<const N: usize> {
    pub value: String<N>,
}

impl<const N: usize> TextField<N> {
    pub const fn new() -> Self {
        Self {
            value: String::new(),
        }
    }

    /// Return true when the event changes the field.
    pub fn apply(&mut self, input: TextInput) -> bool {
        match input {
            TextInput::Character(c) => self.value.push(c).is_ok(),
            TextInput::Backspace => self.value.pop().is_some(),
        }
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(
        &self,
        display: &mut D,
        bounds: Rectangle,
        style: &Style,
    ) -> Result<(), D::Error> {
        bounds
            .into_styled(
                PrimitiveStyleBuilder::new()
                    .stroke_color(style.foreground)
                    .stroke_width(1)
                    .fill_color(style.background)
                    .build(),
            )
            .draw(display)?;
        let inner = bounds.offset(-2);
        let width = style.font.character_size.width + style.font.character_spacing;
        let count = inner.size.width / width.max(1);
        let start = self
            .value
            .char_indices()
            .rev()
            .take(count as usize)
            .last()
            .map_or(self.value.len(), |(i, _)| i);
        Text::with_baseline(
            &self.value[start..],
            Point::new(inner.top_left.x, inner.center().y),
            style.text(TextRole::Body),
            Baseline::Middle,
        )
        .draw(&mut display.clipped(&inner))?;
        Ok(())
    }
}
