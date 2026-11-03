//! Text with a rounded background for short labels and counts.
//!
//! # Design choices
//!
//! - Measure the label with its role’s font before adding padding so the badge fits
//!   the text.
//! - Give an empty label zero bounds so omitting a count also removes its background.
//!
//! See the [text roles](../../../../docs/ui.md#text-roles).

use embedded_graphics::{
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle, RoundedRectangle},
    text::{Baseline, Text},
};

use super::{Style, TextRole};

pub struct TextBadge<'a> {
    pub position: Point,
    pub label: &'a str,
    pub role: TextRole,
    pub underlined: bool,
}

impl TextBadge<'_> {
    /// Measures the text and includes padding on all four sides.
    pub fn bounds(&self, style: &Style) -> Rectangle {
        if self.label.is_empty() {
            return Rectangle::new(self.position, Size::zero());
        }
        let text = Text::with_baseline(
            self.label,
            Point::zero(),
            style.text(self.role),
            Baseline::Top,
        );
        let inset = style.spacing.badge_inset;
        Rectangle::new(
            self.position,
            text.bounding_box().size.saturating_add(inset + inset),
        )
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(
        &self,
        display: &mut D,
        style: &Style,
    ) -> Result<(), D::Error> {
        if self.label.is_empty() {
            return Ok(());
        }
        let bounds = self.bounds(style);
        let mut display = display.clipped(&bounds);
        RoundedRectangle::with_equal_corners(bounds, Size::new_equal(style.badge_radius))
            .into_styled(PrimitiveStyle::with_fill(style.foreground))
            .draw(&mut display)?;
        let mut text = if self.underlined {
            style.underlined_text(self.role)
        } else {
            style.text(self.role)
        };
        text.text_color = Some(style.background);
        let inset = style.spacing.badge_inset;
        Text::with_baseline(
            self.label,
            self.position + Point::new(inset.width as i32, inset.height as i32),
            text,
            Baseline::Top,
        )
        .draw(&mut display)?;
        Ok(())
    }
}
