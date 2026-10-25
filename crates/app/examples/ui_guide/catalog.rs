//! Visual examples that use the application style and controls.

use super::{STYLE, line};
use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle, text::Text};
use paper_mesh_platform::{
    TouchEvent,
    ui::{
        Button, NavigationCard, Page, PageEvent, PageInfo, Presentation, TextRole,
        format::measurement,
    },
};

#[derive(Clone, Copy)]
pub enum Sheet {
    Overview,
    Text,
    Controls,
}
pub struct Catalog(pub Sheet);

impl Page for Catalog {
    type Id = u8;
    fn page_info(&self) -> PageInfo<u8> {
        PageInfo {
            id: 0,
            title: match self.0 {
                Sheet::Overview => "Visual language",
                Sheet::Text => "Text and spacing",
                Sheet::Controls => "Controls",
            },
            context: "PLATFORM UI",
            presentation: Presentation::Standard,
            navigation_enabled: true,
        }
    }
    fn enter(&mut self) {}
    fn leave(&mut self) {}
    fn handle_touch(&mut self, _: TouchEvent) -> PageEvent<u8> {
        PageEvent::None
    }
    fn render_page<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        match self.0 {
            Sheet::Overview => {
                text_roles(display)?;
                controls(display, 330)?;
                spacing(display, 704)?;
            }
            Sheet::Text => {
                text_roles(display)?;
                line(
                    display,
                    "Use Body for status messages.",
                    340,
                    TextRole::Body,
                )?;
                line(
                    display,
                    "Controls select their own label style.",
                    376,
                    TextRole::Body,
                )?;
                spacing(display, 464)?;
                line(
                    display,
                    "Keep essential information readable.",
                    712,
                    TextRole::Body,
                )?;
                line(
                    display,
                    "Use words to explain each state.",
                    748,
                    TextRole::Body,
                )?;
            }
            Sheet::Controls => {
                controls(display, 130)?;
                line(
                    display,
                    "Activate after a complete tap.",
                    552,
                    TextRole::Body,
                )?;
                line(
                    display,
                    "Move outside to cancel the tap.",
                    588,
                    TextRole::Body,
                )?;
                line(
                    display,
                    "Disabled controls cannot activate.",
                    624,
                    TextRole::Body,
                )?;
                line(
                    display,
                    "A pending label explains the wait.",
                    660,
                    TextRole::Body,
                )?;
            }
        }
        Ok(())
    }
}

fn text_roles<D: DrawTarget<Color = Gray4>>(display: &mut D) -> Result<(), D::Error> {
    for (role, text, y) in [
        (TextRole::Caption, "TEXT ROLES", 130),
        (TextRole::Title, "Title: page and section names", 168),
        (TextRole::Body, "Body: instructions, values, status", 204),
        (
            TextRole::Caption,
            "CAPTION: PAGE CONTEXT AND GROUP LABELS",
            240,
        ),
        (
            TextRole::Secondary,
            "Secondary: supporting information",
            276,
        ),
    ] {
        line(display, text, y, role)?;
    }
    Ok(())
}

fn controls<D: DrawTarget<Color = Gray4>>(display: &mut D, y: i32) -> Result<(), D::Error> {
    line(display, "CONTROL STATES", y, TextRole::Caption)?;
    for (x, offset, label, selected, enabled) in [
        (28, 22, "Action", false, true),
        (276, 22, "Selected", true, true),
        (28, 94, "Unavailable", false, false),
        (276, 94, "Pending", true, false),
    ] {
        // guide:button:start
        Button {
            bounds: Rectangle::new(Point::new(x, y + offset), Size::new(236, 56)),
            label,
            selected,
            enabled,
        }
        .render(display, &STYLE)?;
        // guide:button:end
    }
    line(display, "DESTINATION CARD", y + 192, TextRole::Caption)?;
    // guide:card:start
    NavigationCard {
        bounds: Rectangle::new(Point::new(28, y + 220), Size::new(484, 112)),
        title: "Component details",
        description: "A title and a short description",
        enabled: true,
    }
    .render(display, &STYLE)?;
    // guide:card:end
    Ok(())
}

fn spacing<D: DrawTarget<Color = Gray4>>(display: &mut D, y: i32) -> Result<(), D::Error> {
    line(display, "SPACING AND GRAYSCALE", y, TextRole::Caption)?;
    for (label, value, offset) in [
        ("Control gap", STYLE.spacing.control_gap, 36),
        ("Section gap", STYLE.spacing.section_gap, 72),
    ] {
        line(display, label, y + offset, TextRole::Body)?;
        Text::new(
            &measurement(Some(value), "px"),
            Point::new(280, y + offset),
            STYLE.text(TextRole::Body),
        )
        .draw(display)?;
    }
    for (index, color) in [
        STYLE.foreground,
        STYLE.data_fill,
        STYLE.secondary_text,
        STYLE.disabled_text,
        STYLE.divider,
        STYLE.alternating_row,
        STYLE.control_fill,
        STYLE.background,
    ]
    .into_iter()
    .enumerate()
    {
        let x = 28 + index as i32 * 60;
        display.fill_solid(
            &Rectangle::new(Point::new(x, y + 110), Size::new(52, 40)),
            color,
        )?;
        Text::new(
            &measurement(Some(color.luma()), ""),
            Point::new(x, y + 166),
            STYLE.text(TextRole::Caption),
        )
        .draw(display)?;
    }
    Ok(())
}
