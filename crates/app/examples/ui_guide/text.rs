//! Interactive examples of text, spacing, and color roles.
//!
//! # Design choices
//!
//! - Render the selected visual roles with the shared profile so the samples track
//!   changes to that profile.
//!
//! See [Visual language](../../../../docs/ui.md#visual-language).

use embedded_graphics::{
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};
use paper_mesh_platform::ui::EnabledState;
use paper_mesh_platform::ui::{
    Button, Navigator, Page, PageEvent, PageInfo, Presentation, Shell, TextBadge, TextRole,
    format::measurement,
};

use super::{LAYOUT, STYLE, line};

pub const TEXT_ROLES: [(TextRole, &str, &str); 4] = [
    (TextRole::Title, "Title", "Page and section names"),
    (TextRole::Body, "Body", "Instructions, values, and status"),
    (
        TextRole::Caption,
        "Caption",
        "Page context and group labels",
    ),
    (TextRole::Secondary, "Secondary", "Supporting information"),
];
pub const SPACING: [(&str, u32); 2] = [
    ("Control gap", STYLE.spacing.control_gap),
    ("Section gap", STYLE.spacing.section_gap),
];
pub const COLORS: [(&str, Gray4); 8] = [
    ("Foreground", STYLE.foreground),
    ("Data fill", STYLE.data_fill),
    ("Secondary text", STYLE.secondary_text),
    ("Disabled text", STYLE.disabled_text),
    ("Divider", STYLE.divider),
    ("Alternating row", STYLE.alternating_row),
    ("Control fill", STYLE.control_fill),
    ("Background", STYLE.background),
];

pub struct TextPage {
    pub role: usize,
    pub underlined: bool,
    pub bubbled: bool,
    pub spacing: usize,
    pub color: usize,
    elements: paper_mesh_platform::ui::elements::Elements<TextPage, usize, 17, 0>,
}

impl Default for TextPage {
    fn default() -> Self {
        Self {
            role: 1,
            underlined: false,
            bubbled: false,
            spacing: 0,
            color: 0,
            elements: paper_mesh_platform::ui::elements::Elements::new(super::LAYOUT.viewport),
        }
    }
}

impl TextPage {
    pub fn role_buttons(&self) -> [Button<'static>; 4] {
        core::array::from_fn(|index| Button {
            bounds: Rectangle::new(
                Point::new(28 + (index % 2) as i32 * 248, 152 + (index / 2) as i32 * 72),
                Size::new(236, 56),
            ),
            label: TEXT_ROLES[index].1,
            selected: self.role == index,
            enabled: true,
        })
    }

    pub fn underline_button(&self) -> Button<'static> {
        Button {
            bounds: Rectangle::new(Point::new(28, 296), Size::new(236, 52)),
            label: "Underline",
            selected: self.underlined,
            enabled: true,
        }
    }

    pub fn bubble_button(&self) -> Button<'static> {
        Button {
            bounds: Rectangle::new(Point::new(276, 296), Size::new(236, 52)),
            label: "Bubbled",
            selected: self.bubbled,
            enabled: true,
        }
    }

    pub fn spacing_buttons(&self) -> [Button<'static>; 2] {
        core::array::from_fn(|index| Button {
            bounds: Rectangle::new(Point::new(28 + index as i32 * 248, 492), Size::new(236, 56)),
            label: SPACING[index].0,
            selected: self.spacing == index,
            enabled: true,
        })
    }

    pub fn swatches(&self) -> [Button<'static>; 8] {
        core::array::from_fn(|index| Button {
            bounds: Rectangle::new(Point::new(28 + index as i32 * 60, 738), Size::new(52, 56)),
            label: COLORS[index].0,
            selected: self.color == index,
            enabled: true,
        })
    }

    // guide:text:start
    fn render_sample<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        let (role, _, purpose) = TEXT_ROLES[self.role];
        let style = if self.underlined {
            STYLE.underlined_text(role)
        } else {
            STYLE.text(role)
        };
        for (label, x) in [("New", 28), ("3", 144), ("12", 228), ("99+", 322)] {
            let position = Point::new(x, 364);
            if self.bubbled {
                TextBadge {
                    position,
                    label,
                    role,
                    underlined: self.underlined,
                }
                .render(display, &STYLE)?;
            } else {
                let inset = STYLE.spacing.badge_inset;
                Text::with_baseline(
                    label,
                    position + Point::new(inset.width as i32, inset.height as i32),
                    style,
                    Baseline::Top,
                )
                .draw(display)?;
            }
        }
        Text::new(purpose, Point::new(28, 424), STYLE.text(TextRole::Body)).draw(display)?;
        Ok(())
    }
    // guide:text:end
}

impl Page for TextPage {
    type Id = u8;
    fn page_info(&self) -> PageInfo<u8> {
        PageInfo {
            id: 0,
            title: "Text and spacing",
            context: "PLATFORM UI",
            presentation: Presentation::Standard,
            navigation_enabled: true,
        }
    }
    fn did_enter(&mut self) {}
    fn will_leave(&mut self) {}
    type Elements = paper_mesh_platform::ui::elements::Elements<Self, usize, 17, 0>;
    fn elements(&self) -> &Self::Elements {
        &self.elements
    }
    fn elements_mut(&mut self) -> &mut Self::Elements {
        &mut self.elements
    }
    fn sync_elements(&mut self) {
        let roles = self.role_buttons();
        let spacing = self.spacing_buttons();
        let swatches = self.swatches();
        let underline = self.underline_button();
        let bubble = self.bubble_button();
        self.elements
            .surface(
                roles
                    .into_iter()
                    .chain([underline, bubble])
                    .chain(spacing)
                    .chain(swatches)
                    .enumerate()
                    .map(|(i, b)| (b.bounds, i, EnabledState::from(b.enabled))),
            )
            .expect("fixed surface fits element capacity");
    }
    fn configure_handlers(&mut self) {
        self.elements_mut()
            .default_callback(|page, context, action| {
                let result = (|| {
                    match Some(action) {
                        Some(index @ 0..=3) => page.role = index,
                        Some(4) => page.underlined = !page.underlined,
                        Some(5) => page.bubbled = !page.bubbled,
                        Some(index @ 6..=7) => page.spacing = index - 6,
                        Some(index @ 8..=15) => page.color = index - 8,
                        _ => return PageEvent::None,
                    }
                    PageEvent::Changed
                })();
                context.respond(result);
            });
    }
    fn draw_custom<D: DrawTarget<Color = Gray4>>(
        &self,
        _: u16,
        display: &mut D,
    ) -> Result<(), D::Error> {
        line(display, "TEXT ROLES", 130, TextRole::Caption)?;
        for button in self.role_buttons() {
            button.render(display, &STYLE)?;
        }
        self.underline_button().render(display, &STYLE)?;
        self.bubble_button().render(display, &STYLE)?;
        self.render_sample(display)?;
        line(display, "SPACING", 470, TextRole::Caption)?;
        for button in self.spacing_buttons() {
            button.render(display, &STYLE)?;
        }
        let (name, gap) = SPACING[self.spacing];
        for x in [28, 188 + gap as i32] {
            Rectangle::new(Point::new(x, 576), Size::new(160, 56))
                .into_styled(PrimitiveStyle::with_fill(STYLE.control_fill))
                .draw(display)?;
        }
        line(display, name, 664, TextRole::Body)?;
        Text::new(
            &measurement(Some(gap), "px"),
            Point::new(280, 664),
            STYLE.text(TextRole::Body),
        )
        .draw(display)?;
        line(display, "COLOR ROLES", 716, TextRole::Caption)?;
        for (index, swatch) in self.swatches().into_iter().enumerate() {
            swatch
                .bounds
                .into_styled(PrimitiveStyle::with_stroke(STYLE.divider, 1))
                .draw(display)?;
            swatch
                .bounds
                .offset(-4)
                .into_styled(PrimitiveStyle::with_fill(COLORS[index].1))
                .draw(display)?;
            if swatch.selected {
                Rectangle::new(swatch.bounds.top_left + Point::new(0, 60), Size::new(52, 3))
                    .into_styled(PrimitiveStyle::with_fill(STYLE.foreground))
                    .draw(display)?;
            }
        }
        let (name, color) = COLORS[self.color];
        line(display, name, 844, TextRole::Body)?;
        Text::new(
            &measurement(Some(color.luma()), " / 15"),
            Point::new(280, 844),
            STYLE.text(TextRole::Body),
        )
        .draw(display)?;
        line(
            display,
            "Tap a swatch to inspect its role.",
            892,
            TextRole::Secondary,
        )
    }
}

pub struct TextExample {
    pub ui: Navigator<TextPage, 0>,
}

impl Default for TextExample {
    fn default() -> Self {
        Self {
            ui: Navigator::launch(0, Shell::new(LAYOUT), TextPage::default()),
        }
    }
}
