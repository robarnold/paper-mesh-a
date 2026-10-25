//! Bounded controls with shared rendering and hit-test state.

use embedded_graphics::{
    mono_font::MonoTextStyle,
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};

use super::Style;

#[derive(Clone, Copy)]
pub struct Button<'a> {
    pub bounds: Rectangle,
    pub label: &'a str,
    pub selected: bool,
    pub enabled: bool,
}

impl Button<'_> {
    pub fn hit(&self, point: Point) -> bool {
        self.enabled && self.bounds.contains(point)
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(
        &self,
        display: &mut D,
        style: &Style,
    ) -> Result<(), D::Error> {
        let fill = if self.selected && self.enabled {
            style.foreground
        } else {
            style.control_fill
        };
        let foreground = if !self.enabled {
            style.disabled_text
        } else if self.selected {
            style.background
        } else {
            style.foreground
        };
        self.bounds
            .into_styled(PrimitiveStyle::with_fill(fill))
            .draw(display)?;
        let inset = style.spacing.control_inset.min(self.bounds.size.width / 2);
        let text_bounds = Rectangle::new(
            self.bounds.top_left + Point::new(inset as i32, 0),
            Size::new(
                self.bounds.size.width.saturating_sub(inset),
                self.bounds.size.height,
            ),
        );
        let position = text_bounds.top_left
            + Point::new(
                0,
                text_bounds
                    .size
                    .height
                    .saturating_sub(style.font.character_size.height) as i32
                    / 2,
            );
        Text::with_baseline(
            self.label,
            position,
            MonoTextStyle::new(style.font, foreground),
            Baseline::Top,
        )
        .draw(&mut display.clipped(&text_bounds))?;
        Ok(())
    }
}

pub struct NavigationCard<'a> {
    pub bounds: Rectangle,
    pub title: &'a str,
    pub description: &'a str,
    pub enabled: bool,
}

impl NavigationCard<'_> {
    pub fn hit(&self, point: Point) -> bool {
        self.enabled && self.bounds.contains(point)
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(
        &self,
        display: &mut D,
        style: &Style,
    ) -> Result<(), D::Error> {
        let foreground = if self.enabled {
            style.foreground
        } else {
            style.disabled_text
        };
        // Clip the border as well as text to the caller's bounds.
        let mut display = display.clipped(&self.bounds);
        self.bounds
            .into_styled(PrimitiveStyle::with_stroke(foreground, 2))
            .draw(&mut display)?;
        let inset = style
            .spacing
            .card_inset
            .min(self.bounds.size.width / 2)
            .min(self.bounds.size.height / 2);
        let content = Rectangle::new(
            self.bounds.top_left + Point::new(inset as i32, inset as i32),
            self.bounds
                .size
                .saturating_sub(Size::new(2 * inset, 2 * inset)),
        );
        let font = MonoTextStyle::new(style.font, foreground);
        let arrow_width = style.font.character_size.width;
        let title = Rectangle::new(
            content.top_left,
            Size::new(
                content.size.width.saturating_sub(2 * arrow_width),
                style.font.character_size.height.min(content.size.height),
            ),
        );
        Text::with_baseline(self.title, title.top_left, font, Baseline::Top)
            .draw(&mut display.clipped(&title))?;
        let mut display = display.clipped(&content);
        Text::with_baseline(
            ">",
            content.top_left + Point::new(content.size.width.saturating_sub(arrow_width) as i32, 0),
            font,
            Baseline::Top,
        )
        .draw(&mut display)?;
        Text::with_baseline(
            self.description,
            content.top_left
                + Point::new(
                    0,
                    content
                        .size
                        .height
                        .saturating_sub(style.font.character_size.height)
                        as i32,
                ),
            font,
            Baseline::Top,
        )
        .draw(&mut display)?;
        Ok(())
    }
}
