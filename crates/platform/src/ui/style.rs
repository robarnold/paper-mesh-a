//! Semantic visual roles for grayscale interfaces.

use embedded_graphics::{
    mono_font::{
        MonoFont, MonoTextStyle,
        ascii::{FONT_6X10, FONT_10X20},
    },
    pixelcolor::Gray4,
    prelude::*,
    text::DecorationColor,
};

#[derive(Clone, Copy)]
pub enum TextRole {
    Title,
    Body,
    Caption,
    Secondary,
}

#[derive(Clone, Copy)]
pub struct Spacing {
    pub badge_inset: Size,
    pub control_inset: u32,
    pub card_inset: u32,
    pub control_gap: u32,
    pub section_gap: u32,
    pub minimum_control_height: u32,
}

#[derive(Clone, Copy)]
pub struct Style {
    pub title_font: &'static MonoFont<'static>,
    pub font: &'static MonoFont<'static>,
    pub caption_font: &'static MonoFont<'static>,
    pub foreground: Gray4,
    pub background: Gray4,
    pub secondary_text: Gray4,
    pub divider: Gray4,
    pub control_fill: Gray4,
    pub disabled_text: Gray4,
    pub data_fill: Gray4,
    pub alternating_row: Gray4,
    pub badge_radius: u32,
    pub spacing: Spacing,
}

impl Style {
    pub const GRAYSCALE: Self = Self {
        title_font: &FONT_10X20,
        font: &FONT_10X20,
        caption_font: &FONT_6X10,
        foreground: Gray4::BLACK,
        background: Gray4::WHITE,
        secondary_text: Gray4::new(6),
        divider: Gray4::new(9),
        control_fill: Gray4::new(14),
        disabled_text: Gray4::new(8),
        data_fill: Gray4::new(5),
        alternating_row: Gray4::new(12),
        badge_radius: 8,
        spacing: Spacing {
            badge_inset: Size::new(8, 4),
            control_inset: 16,
            card_inset: 20,
            control_gap: 12,
            section_gap: 28,
            minimum_control_height: 52,
        },
    };

    pub fn text(&self, role: TextRole) -> MonoTextStyle<'static, Gray4> {
        let font = match role {
            TextRole::Title => self.title_font,
            TextRole::Caption => self.caption_font,
            _ => self.font,
        };
        let color = if matches!(role, TextRole::Secondary) {
            self.secondary_text
        } else {
            self.foreground
        };
        MonoTextStyle::new(font, color)
    }

    /// Adds an underline in the role's text color without changing its font.
    pub fn underlined_text(&self, role: TextRole) -> MonoTextStyle<'static, Gray4> {
        let mut style = self.text(role);
        style.underline_color = DecorationColor::TextColor;
        style
    }
}
