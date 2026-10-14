#![no_std]

//! Shared application state and rendering.
//! This crate operates without ESP registers, an OS, a heap, or an executor.
//! Each platform delivers input and schedules application polls.

pub mod platform;

use core::fmt::Write;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Alignment, Text},
};
use heapless::String;
use platform::{Clock, Display, SCREEN_SIZE, TouchEvent};

pub const HEARTBEAT_MS: u64 = 30_000;
const MARGIN: u32 = 48;
pub const TAP_TARGET: Rectangle = Rectangle::new(
    Point::new(MARGIN as i32, SCREEN_SIZE.height as i32 / 2 - 100),
    Size::new(SCREEN_SIZE.width - 2 * MARGIN, 200),
);

pub struct App {
    started_at_ms: u64,
    last_heartbeat_ms: u64,
    uptime_seconds: u64,
    taps: u32,
    touch_armed: bool,
    dirty: bool,
}

impl App {
    pub fn new(clock: &impl Clock) -> Self {
        let now = clock.now_ms();
        Self {
            started_at_ms: now,
            last_heartbeat_ms: now,
            uptime_seconds: 0,
            taps: 0,
            touch_armed: false,
            dirty: true,
        }
    }

    /// Emits at most one heartbeat per poll.
    /// A long pause does not change the initial schedule.
    /// The platform supplies time through the clock argument.
    pub fn tick(&mut self, clock: &impl Clock) -> Option<u64> {
        let now = clock.now_ms();
        let elapsed = now.saturating_sub(self.last_heartbeat_ms);
        if elapsed < HEARTBEAT_MS {
            return None;
        }
        self.last_heartbeat_ms += (elapsed / HEARTBEAT_MS) * HEARTBEAT_MS;
        self.uptime_seconds = now.saturating_sub(self.started_at_ms) / 1000;
        self.dirty = true;
        Some(self.uptime_seconds)
    }

    pub fn next_deadline_ms(&self) -> u64 {
        self.last_heartbeat_ms.saturating_add(HEARTBEAT_MS)
    }

    pub fn taps(&self) -> u32 {
        self.taps
    }

    pub fn touch(&mut self, event: TouchEvent) {
        match event {
            TouchEvent::Down(point) => self.touch_armed = TAP_TARGET.contains(point),
            TouchEvent::Up(point) => {
                if self.touch_armed && TAP_TARGET.contains(point) {
                    self.taps = self.taps.saturating_add(1);
                    self.dirty = true;
                }
                self.touch_armed = false;
            }
            TouchEvent::Cancel => self.touch_armed = false,
        }
    }

    /// Combines changes while the panel is busy.
    /// Returns true only after successful submission of a new frame.
    /// A drawing or refresh error leaves the frame pending.
    pub fn present<D: Display>(&mut self, display: &mut D) -> Result<bool, D::Error> {
        if !self.dirty || display.is_busy() {
            return Ok(false);
        }
        self.render(display)?;
        display.refresh()?;
        self.dirty = false;
        Ok(true)
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        display.clear(Gray4::WHITE)?;
        let black = MonoTextStyle::new(&FONT_10X20, Gray4::BLACK);
        let muted = MonoTextStyle::new(&FONT_10X20, Gray4::new(6));
        Text::new("PAPER MESH", Point::new(48, 60), black).draw(display)?;
        Text::new("Hello from shared Rust", Point::new(48, 110), muted).draw(display)?;
        Rectangle::new(Point::new(48, 144), Size::new(TAP_TARGET.size.width, 2))
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(11)))
            .draw(display)?;
        TAP_TARGET
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(display)?;
        Text::with_alignment(
            "TAP TO COUNT",
            Point::new(SCREEN_SIZE.width as i32 / 2, TAP_TARGET.top_left.y + 75),
            MonoTextStyle::new(&FONT_10X20, Gray4::WHITE),
            Alignment::Center,
        )
        .draw(display)?;
        let mut label = String::<64>::new();
        // Decimal u32/u64 widths limit the formatted lengths.
        let _ = write!(label, "Touches: {}", self.taps);
        Text::with_alignment(
            &label,
            Point::new(SCREEN_SIZE.width as i32 / 2, TAP_TARGET.top_left.y + 130),
            MonoTextStyle::new(&FONT_10X20, Gray4::WHITE),
            Alignment::Center,
        )
        .draw(display)?;
        label.clear();
        let _ = write!(label, "Uptime: {}s", self.uptime_seconds);
        Text::new(
            &label,
            Point::new(48, SCREEN_SIZE.height as i32 - 208),
            black,
        )
        .draw(display)?;
        label.clear();
        let _ = write!(
            label,
            "{} x {} / 16 shades",
            SCREEN_SIZE.width, SCREEN_SIZE.height
        );
        Text::new(
            &label,
            Point::new(48, SCREEN_SIZE.height as i32 - 128),
            muted,
        )
        .draw(display)?;
        for shade in 0..16 {
            Rectangle::new(
                Point::new(48 + shade * 20, SCREEN_SIZE.height as i32 - 88),
                Size::new(20, 20),
            )
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(shade as u8)))
            .draw(display)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestClock(u64);
    impl Clock for TestClock {
        fn now_ms(&self) -> u64 {
            self.0
        }
    }

    #[test]
    fn heartbeat_catches_up_without_drift_or_a_burst() {
        let mut clock = TestClock(500);
        let mut app = App::new(&clock);
        clock.0 = 30_499;
        assert_eq!(app.tick(&clock), None);
        clock.0 = 95_500;
        assert_eq!(app.tick(&clock), Some(95));
        assert_eq!(app.next_deadline_ms(), 120_500);
        assert_eq!(app.tick(&clock), None);
        clock.0 = 120_500;
        assert_eq!(app.tick(&clock), Some(120));
    }

    #[test]
    fn touches_require_a_complete_gesture_inside_the_button() {
        let mut app = App::new(&TestClock(0));
        let inside = TAP_TARGET.center();
        let outside = Point::new(0, 0);
        app.touch(TouchEvent::Up(inside));
        app.touch(TouchEvent::Down(outside));
        app.touch(TouchEvent::Up(inside));
        app.touch(TouchEvent::Down(inside));
        app.touch(TouchEvent::Up(outside));
        app.touch(TouchEvent::Down(inside));
        app.touch(TouchEvent::Cancel);
        app.touch(TouchEvent::Up(inside));
        assert_eq!(app.taps(), 0);
        app.touch(TouchEvent::Down(inside));
        app.touch(TouchEvent::Up(inside));
        assert_eq!(app.taps(), 1);
    }
}
