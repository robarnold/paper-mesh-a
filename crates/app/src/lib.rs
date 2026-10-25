#![no_std]

//! Shared application state and rendering.
//! This crate operates without ESP registers, an OS, a heap, or an executor.
//! Each platform delivers input and schedules application polls.

pub mod debugger;
pub mod diagnostics;
pub mod display_check;
mod home;
pub mod location;
pub mod pages;
pub mod power;
pub mod power_off;
pub mod screen;
pub mod suspend;
pub mod touch;
pub mod touchpad;

pub use screen::BACK_TARGET;
pub use touch::TAP_TARGET;

use debugger::Debugger;
use diagnostics::Component;
use embedded_graphics::{pixelcolor::Gray4, prelude::*};
use location::LocationScreen;
use pages::{Pages, Route};
use paper_mesh_platform::discovery::Discovery;
use paper_mesh_platform::location::Location;
use paper_mesh_platform::power::Power;
use paper_mesh_platform::{
    Clock, Display, TouchEvent, display,
    time::PeriodicTimer,
    ui::{Navigation, Navigator, PageInfo, Presentation, Shell},
};
use power::PowerScreen;

/// Fixed logical viewport for this application. Backends supply a compatible display.
pub const SCREEN_SIZE: Size = Size::new(540, 960);

pub const HEARTBEAT_MS: u64 = 30_000;

pub struct App {
    ui: Navigator<Pages, 8>,
    full_refresh: bool,
    power_off_control: Option<paper_mesh_platform::input::buttons::ButtonId>,
    power_off_deadline: Option<u64>,
    started_at_ms: u64,
    heartbeat: PeriodicTimer,
    dirty: bool,
}

impl App {
    pub fn new(clock: &impl Clock) -> Self {
        Self::new_at(clock.now_ms())
    }

    /// Launch the application at the specified time in the platform epoch.
    pub fn new_at(now: u64) -> Self {
        Self {
            ui: Navigator::launch(Route::Home, Shell::new(screen::LAYOUT), Pages::new()),
            full_refresh: false,
            power_off_control: None,
            power_off_deadline: None,
            started_at_ms: now,
            heartbeat: PeriodicTimer::new(now, HEARTBEAT_MS),
            dirty: true,
        }
    }

    /// Emits at most one heartbeat per poll.
    /// A long pause does not change the initial schedule.
    /// The platform supplies time through the clock argument.
    pub fn tick(&mut self, clock: &impl Clock) -> Option<u64> {
        let now = clock.now_ms();
        if self
            .power_off_deadline
            .is_some_and(|deadline| now >= deadline)
        {
            self.power_off_deadline = None;
            self.navigate(Navigation::Open(Route::PowerOff));
            if self.current_page() == Route::PowerOff {
                self.ui.pages_mut().power.off.request();
                self.dirty = true;
            }
        }
        if !self.heartbeat.poll(now) {
            return None;
        }
        self.ui.pages_mut().home.uptime_seconds = now.saturating_sub(self.started_at_ms) / 1000;
        self.dirty |=
            !self.touchpad_open() && self.ui.pages_mut().display_check.pattern().is_none();
        Some(self.ui.pages_mut().home.uptime_seconds)
    }

    pub fn next_deadline_ms(&self) -> u64 {
        self.ui
            .pages()
            .debugger
            .next_deadline_ms()
            .unwrap_or(u64::MAX)
            .min(self.next_heartbeat_ms())
            .min(
                self.ui
                    .pages()
                    .location
                    .next_deadline_ms()
                    .unwrap_or(u64::MAX),
            )
            .min(self.ui.pages().power.next_deadline_ms().unwrap_or(u64::MAX))
            .min(self.power_off_deadline.unwrap_or(u64::MAX))
    }

    pub fn next_heartbeat_ms(&self) -> u64 {
        self.heartbeat.next_deadline_ms()
    }

    pub fn taps(&self) -> u32 {
        self.ui.pages().touch_screen.taps()
    }

    /// Applies application bindings to backend control identities.
    pub fn hardware_button(
        &mut self,
        event: paper_mesh_platform::input::buttons::ButtonEvent,
        exit_controls: &[paper_mesh_platform::input::buttons::ButtonId],
    ) {
        if self.ui.pages_mut().display_check.pattern().is_some() {
            self.power_off_deadline = None;
            let changed = if exit_controls.contains(&event.id)
                && matches!(
                    event.state,
                    paper_mesh_platform::input::buttons::ButtonState::Pressed
                        | paper_mesh_platform::input::buttons::ButtonState::Activated
                ) {
                self.navigate(Navigation::Back);
                true
            } else {
                self.ui.pages_mut().display_check.advance(event)
            };
            self.dirty |= changed;
            self.full_refresh |= changed;
            return;
        }
        if Some(event.id) == self.power_off_control {
            self.power_off_deadline = match event.state {
                paper_mesh_platform::input::buttons::ButtonState::Pressed => {
                    Some(event.at_ms.saturating_add(3_000))
                }
                _ => None,
            };
        }
        if exit_controls.contains(&event.id)
            && event.state == paper_mesh_platform::input::buttons::ButtonState::Pressed
        {
            self.cancel_touchpad();
        }
    }

    pub fn diagnostics(&self) -> &diagnostics::Menu {
        &self.ui.pages().diagnostics
    }

    pub fn active_diagnostic(&self) -> Option<Component> {
        match self.current_page() {
            Route::Display => Some(Component::Display),
            Route::Touch => Some(Component::Touch),
            Route::Radio => Some(Component::Radio),
            Route::Location => Some(Component::Location),
            Route::Power | Route::Suspend | Route::PowerOff => Some(Component::Power),
            _ => None,
        }
    }

    pub fn display_check(&self) -> &display_check::DisplayCheck {
        &self.ui.pages().display_check
    }

    pub fn touch_screen(&self) -> &touch::TouchScreen {
        &self.ui.pages().touch_screen
    }

    pub fn set_diagnostic_controls(
        &mut self,
        advance: paper_mesh_platform::input::buttons::ButtonDescriptor,
        exit_hint: &'static str,
    ) {
        self.ui
            .pages_mut()
            .display_check
            .set_controls(advance, exit_hint);
    }

    pub fn display_completed(&mut self, at_ms: u64) {
        self.ui.pages_mut().display_check.display_completed(at_ms);
    }

    pub fn display_failed(&mut self) {
        self.ui.pages_mut().display_check.display_failed();
        self.dirty |= self.ui.pages_mut().display_check.is_open();
    }

    pub fn touchpad_open(&self) -> bool {
        self.current_page() == Route::Touchpad
    }

    pub fn cancel_touchpad(&mut self) {
        if self.touchpad_open() {
            self.navigate(Navigation::Home);
        }
    }

    pub fn set_touchpad_host(&mut self, state: paper_mesh_platform::touchpad::HostState) {
        if state != self.ui.pages_mut().touchpad.host {
            self.ui.pages_mut().touchpad.host = state;
            self.dirty |= self.touchpad_open();
        }
    }

    pub fn debugger(&self) -> &Debugger {
        &self.ui.pages().debugger
    }

    pub fn poll_discovery(&mut self, clock: &impl Clock, discovery: &mut impl Discovery) {
        self.dirty |= self.ui.pages_mut().debugger.poll(clock.now_ms(), discovery);
    }

    pub fn location(&self) -> &LocationScreen {
        &self.ui.pages().location
    }

    pub fn poll_location(&mut self, clock: &impl Clock, location: &mut impl Location) {
        self.dirty |= self.ui.pages_mut().location.poll(clock.now_ms(), location);
    }

    pub fn power(&self) -> &PowerScreen {
        &self.ui.pages().power
    }

    pub fn set_suspend_controls(
        &mut self,
        controls: &'static [paper_mesh_platform::input::buttons::ButtonDescriptor],
    ) {
        self.ui.pages_mut().power.suspend.set_controls(controls);
    }
    pub fn take_suspend_request(&mut self) -> Option<paper_mesh_platform::suspend::SuspendRequest> {
        self.ui.pages_mut().power.suspend.take_request()
    }
    pub fn finish_suspend(
        &mut self,
        result: Result<
            paper_mesh_platform::suspend::SuspendReport,
            paper_mesh_platform::suspend::SuspendError,
        >,
    ) {
        self.ui.pages_mut().power.suspend.finish(result);
        self.dirty = true;
    }

    pub fn poll_power(&mut self, clock: &impl Clock, power: &mut impl Power) {
        self.dirty |= self.ui.pages_mut().power.poll(clock.now_ms(), power);
    }

    pub fn set_power_off_restart_hint(&mut self, hint: &'static str) {
        self.ui.pages_mut().power.off.set_restart_hint(hint);
    }

    pub fn set_power_off_control(&mut self, id: paper_mesh_platform::input::buttons::ButtonId) {
        self.power_off_control = Some(id);
        self.power_off_deadline = None;
    }

    pub fn take_power_off_request(&mut self) -> bool {
        self.ui.pages_mut().power.off.take_request()
    }

    pub fn finish_power_off(&mut self, error: paper_mesh_platform::power_off::PowerOffError) {
        self.ui.pages_mut().power.off.finish(error);
        self.dirty = true;
    }

    pub fn current_page(&self) -> Route {
        self.ui.current()
    }

    fn page_info(&self) -> PageInfo<Route> {
        self.ui.page_info()
    }

    pub fn navigation_visible(&self) -> bool {
        self.page_info().presentation == Presentation::Standard
    }

    pub fn navigation_controls(&self) -> Option<paper_mesh_platform::ui::Button<'static>> {
        self.ui.back_control()
    }

    /// The runtime delivers the platform's reserved Home request here.
    pub fn go_home(&mut self) {
        self.power_off_deadline = None;
        self.navigate(Navigation::Home);
    }

    fn navigate(&mut self, navigation: Navigation<Route>) {
        let was_pattern = self.ui.pages().display_check.pattern().is_some();
        if let Ok(Some(_)) = self.ui.navigate(navigation) {
            self.power_off_deadline = None;
            self.full_refresh |= was_pattern;
            self.dirty = true;
        }
    }

    pub fn touch(&mut self, event: TouchEvent) {
        let was_pattern = self.ui.pages().display_check.pattern().is_some();
        if let Ok(update) = self.ui.touch(event) {
            self.dirty |= update.changed;
            if update.changed {
                self.full_refresh |=
                    was_pattern || self.ui.pages().display_check.pattern().is_some();
            }
            if update.transition.is_some() {
                self.power_off_deadline = None;
            }
        }
        if self.ui.pages().display_check.pattern().is_some() {
            self.power_off_deadline = None;
        }
    }

    /// Combines changes while the panel is busy.
    /// Returns true only after successful submission of a new frame.
    /// A drawing or refresh error leaves the frame pending.
    pub fn present<D: Display>(&mut self, display: &mut D) -> Result<bool, D::Error> {
        let submitted = if self.full_refresh {
            display::present_full(display, self.dirty, |target| self.render(target))?
        } else {
            display::present(display, self.dirty, |target| self.render(target))?
        };
        if submitted {
            self.dirty = false;
            self.full_refresh = false;
            self.ui.pages_mut().display_check.submitted();
        }
        Ok(submitted)
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        self.ui.render(display, &screen::STYLE)
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
        app.navigate(Navigation::Open(Route::Touch));
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
