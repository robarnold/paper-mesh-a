#![no_std]

//! Shared application state and rendering.
//! This crate operates without ESP registers, an OS, a heap, or an executor.
//! Each platform delivers input and schedules application polls.

pub mod debugger;
pub mod location;
pub mod power;

use core::fmt::Write;
use debugger::{DEBUGGER_TARGET, Debugger};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Alignment, Text},
};
use heapless::String;
use location::{LOCATION_TARGET, LocationScreen};
use paper_mesh_platform::discovery::Discovery;
use paper_mesh_platform::location::Location;
use paper_mesh_platform::power::Power;
use paper_mesh_platform::{
    Clock, Display, TouchEvent, display, input::TapGesture, time::PeriodicTimer, ui::button,
};
use power::{POWER_TARGET, PowerScreen};

/// Fixed logical viewport for this application. Backends supply a compatible display.
pub const SCREEN_SIZE: Size = Size::new(540, 960);

pub const BACK_TARGET: Rectangle = Rectangle::new(Point::new(402, 24), Size::new(110, 56));

pub const HEARTBEAT_MS: u64 = 30_000;
const MARGIN: u32 = 48;
pub const TAP_TARGET: Rectangle = Rectangle::new(
    Point::new(MARGIN as i32, SCREEN_SIZE.height as i32 / 2 - 100),
    Size::new(SCREEN_SIZE.width - 2 * MARGIN, 200),
);

#[derive(PartialEq)]
enum HomeAction {
    Count,
    Debugger,
    Location,
    Power,
}

pub struct App {
    started_at_ms: u64,
    heartbeat: PeriodicTimer,
    uptime_seconds: u64,
    taps: u32,
    home_gesture: TapGesture<HomeAction>,
    debugger: Debugger,
    location: LocationScreen,
    power: PowerScreen,
    dirty: bool,
}

impl App {
    pub fn new(clock: &impl Clock) -> Self {
        Self::new_at(clock.now_ms())
    }

    /// This constructor permits static storage with a specified start time in the platform epoch.
    pub const fn new_at(now: u64) -> Self {
        Self {
            started_at_ms: now,
            heartbeat: PeriodicTimer::new(now, HEARTBEAT_MS),
            uptime_seconds: 0,
            taps: 0,
            home_gesture: TapGesture::new(),
            debugger: Debugger::new(),
            location: LocationScreen::new(),
            power: PowerScreen::new(),
            dirty: true,
        }
    }

    /// Emits at most one heartbeat per poll.
    /// A long pause does not change the initial schedule.
    /// The platform supplies time through the clock argument.
    pub fn tick(&mut self, clock: &impl Clock) -> Option<u64> {
        let now = clock.now_ms();
        if !self.heartbeat.poll(now) {
            return None;
        }
        self.uptime_seconds = now.saturating_sub(self.started_at_ms) / 1000;
        self.dirty = true;
        Some(self.uptime_seconds)
    }

    pub fn next_deadline_ms(&self) -> u64 {
        self.debugger
            .next_deadline_ms()
            .unwrap_or(u64::MAX)
            .min(self.next_heartbeat_ms())
            .min(self.location.next_deadline_ms().unwrap_or(u64::MAX))
            .min(self.power.next_deadline_ms().unwrap_or(u64::MAX))
    }

    pub fn next_heartbeat_ms(&self) -> u64 {
        self.heartbeat.next_deadline_ms()
    }

    pub fn taps(&self) -> u32 {
        self.taps
    }

    pub fn debugger(&self) -> &Debugger {
        &self.debugger
    }

    pub fn poll_discovery(&mut self, clock: &impl Clock, discovery: &mut impl Discovery) {
        self.dirty |= self.debugger.poll(clock.now_ms(), discovery);
    }

    pub fn location(&self) -> &LocationScreen {
        &self.location
    }

    pub fn poll_location(&mut self, clock: &impl Clock, location: &mut impl Location) {
        self.dirty |= self.location.poll(clock.now_ms(), location);
    }

    pub fn power(&self) -> &PowerScreen {
        &self.power
    }

    pub fn poll_power(&mut self, clock: &impl Clock, power: &mut impl Power) {
        self.dirty |= self.power.poll(clock.now_ms(), power);
    }

    pub fn touch(&mut self, event: TouchEvent) {
        if self.power.is_open() {
            self.dirty |= self.power.touch(event);
            return;
        }
        if self.location.is_open() {
            self.dirty |= self.location.touch(event);
            return;
        }
        if self.debugger.is_open() {
            self.dirty |= self.debugger.touch(event);
            return;
        }
        if let Some(action) = self.home_gesture.update(event, |point| {
            [
                (TAP_TARGET, HomeAction::Count),
                (DEBUGGER_TARGET, HomeAction::Debugger),
                (LOCATION_TARGET, HomeAction::Location),
                (POWER_TARGET, HomeAction::Power),
            ]
            .into_iter()
            .find_map(|(target, action)| target.contains(point).then_some(action))
        }) {
            match action {
                HomeAction::Count => self.taps = self.taps.saturating_add(1),
                HomeAction::Debugger => self.debugger.open(),
                HomeAction::Location => self.location.open(),
                HomeAction::Power => self.power.open(),
            }
            self.dirty = true;
        }
    }

    /// Combines changes while the panel is busy.
    /// Returns true only after successful submission of a new frame.
    /// A drawing or refresh error leaves the frame pending.
    pub fn present<D: Display>(&mut self, display: &mut D) -> Result<bool, D::Error> {
        let submitted = display::present(display, self.dirty, |target| self.render(target))?;
        if submitted {
            self.dirty = false;
        }
        Ok(submitted)
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        if self.power.is_open() {
            return self.power.render(display);
        }
        if self.location.is_open() {
            return self.location.render(display);
        }
        if self.debugger.is_open() {
            return self.debugger.render(display);
        }
        display.clear(Gray4::WHITE)?;
        let black = MonoTextStyle::new(&FONT_10X20, Gray4::BLACK);
        let muted = MonoTextStyle::new(&FONT_10X20, Gray4::new(6));
        Text::new("PAPER MESH", Point::new(48, 60), black).draw(display)?;
        Text::new("Hello from shared Rust", Point::new(48, 110), muted).draw(display)?;
        button(display, DEBUGGER_TARGET, "Radio debugger", false, true)?;
        button(display, LOCATION_TARGET, "GNSS", false, true)?;
        button(display, POWER_TARGET, "Power", false, true)?;
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
        app.touch(TouchEvent::release(0, inside));
        app.touch(TouchEvent::touch(0, outside));
        app.touch(TouchEvent::release(0, inside));
        app.touch(TouchEvent::touch(0, inside));
        app.touch(TouchEvent::release(0, outside));
        app.touch(TouchEvent::touch(0, inside));
        app.touch(TouchEvent::Cancel);
        app.touch(TouchEvent::release(0, inside));
        assert_eq!(app.taps(), 0);
        app.touch(TouchEvent::touch(0, inside));
        app.touch(TouchEvent::release(0, inside));
        assert_eq!(app.taps(), 1);
    }
}
