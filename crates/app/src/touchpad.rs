//! Touchpad application screen and activation policy.

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Gray4,
    prelude::*,
    primitives::Rectangle,
    text::Text,
};
use paper_mesh_platform::touchpad::HostState;

pub const TOUCHPAD_TARGET: Rectangle = Rectangle::new(Point::new(48, 700), Size::new(444, 64));

pub fn render<D: DrawTarget<Color = Gray4>>(
    display: &mut D,
    state: HostState,
) -> Result<(), D::Error> {
    display.clear(Gray4::WHITE)?;
    let text = MonoTextStyle::new(&FONT_10X20, Gray4::BLACK);
    for (label, y) in [
        ("USB TOUCHPAD", 72),
        ("Move fingers on this surface.", 180),
        ("Linux controls taps and gestures.", 220),
        ("Press Function or Home to exit.", 300),
        (status(state), 380),
    ] {
        Text::new(label, Point::new(48, y), text).draw(display)?;
    }
    Ok(())
}

pub fn status(state: HostState) -> &'static str {
    match state {
        HostState::Ready => "USB host ready",
        HostState::Disconnected => "Connect a USB host",
        HostState::Suspended => "USB host suspended",
        HostState::Fault => "USB input fault",
    }
}
