//! Application controls for a small, manually advanced display check.

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Gray4,
    prelude::*,
    primitives::Rectangle,
    text::Text,
};
use paper_mesh_platform::{
    TouchEvent,
    input::{
        TapGesture,
        buttons::{ButtonDescriptor, ButtonEvent, ButtonState},
    },
    ui::button,
};

use crate::BACK_TARGET;

pub const HOME_TARGET: Rectangle = Rectangle::new(Point::new(330, 30), Size::new(182, 56));
pub const START: Rectangle = Rectangle::new(Point::new(28, 420), Size::new(484, 64));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Black,
    White,
    Gray,
}

impl Pattern {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Black => "black",
            Self::White => "white",
            Self::Gray => "gray",
        }
    }
    pub const fn color(self) -> Gray4 {
        match self {
            Self::Black => Gray4::BLACK,
            Self::White => Gray4::WHITE,
            Self::Gray => Gray4::new(8),
        }
    }
}

#[derive(PartialEq)]
enum Action {
    Back,
    Start,
}

pub struct Diagnostics {
    open: bool,
    pattern: Option<Pattern>,
    submitted: bool,
    ready_after_ms: Option<u64>,
    pressed: bool,
    complete: bool,
    available: bool,
    advance: Option<ButtonDescriptor>,
    exit_hint: &'static str,
    gesture: TapGesture<Action>,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new()
    }
}

impl Diagnostics {
    pub const fn new() -> Self {
        Self {
            open: false,
            pattern: None,
            submitted: false,
            ready_after_ms: None,
            pressed: false,
            complete: false,
            available: true,
            advance: None,
            exit_hint: "",
            gesture: TapGesture::new(),
        }
    }

    pub fn set_controls(&mut self, advance: ButtonDescriptor, exit_hint: &'static str) {
        self.advance = Some(advance);
        self.exit_hint = exit_hint;
    }

    pub fn open(&mut self) {
        self.open = true;
        self.complete = false;
        self.gesture.cancel();
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn pattern(&self) -> Option<Pattern> {
        self.pattern
    }
    pub fn held_pattern(&self) -> Option<Pattern> {
        self.ready_after_ms.and(self.pattern)
    }

    fn select(&mut self, pattern: Option<Pattern>) {
        self.pattern = pattern;
        self.submitted = false;
        self.ready_after_ms = None;
        self.pressed = false;
        self.gesture.cancel();
    }

    pub fn close(&mut self) {
        self.select(None);
        self.open = false;
    }

    pub fn display_failed(&mut self) {
        self.select(None);
        self.available = false;
    }

    pub fn submitted(&mut self) {
        if self.pattern.is_some() {
            self.submitted = true;
        }
    }

    /// The runtime reports completion only after it regains an idle display.
    pub fn display_completed(&mut self, at_ms: u64) {
        if self.submitted && self.ready_after_ms.is_none() {
            self.ready_after_ms = Some(at_ms);
        }
    }

    /// Ignore queued events, startup holds, and presses during a refresh.
    pub fn advance(&mut self, event: ButtonEvent) -> bool {
        if self.advance.is_none_or(|control| control.id != event.id) {
            return false;
        }
        if self.ready_after_ms.is_none_or(|ready| event.at_ms <= ready) {
            self.pressed = false;
            return false;
        }
        match event.state {
            ButtonState::Pressed => self.pressed = true,
            ButtonState::Released if self.pressed => {
                let next = match self.pattern {
                    Some(Pattern::Black) => Some(Pattern::White),
                    Some(Pattern::White) => Some(Pattern::Gray),
                    Some(Pattern::Gray) => {
                        self.complete = true;
                        None
                    }
                    None => return false,
                };
                self.select(next);
                return true;
            }
            _ => self.pressed = false,
        }
        false
    }

    pub fn touch(&mut self, event: TouchEvent) -> bool {
        if self.pattern.is_some() {
            return false;
        }
        match self.gesture.update(event, |point| {
            if BACK_TARGET.contains(point) {
                Some(Action::Back)
            } else if self.available && self.advance.is_some() && START.contains(point) {
                Some(Action::Start)
            } else {
                None
            }
        }) {
            Some(Action::Back) => self.close(),
            Some(Action::Start) => {
                self.complete = false;
                self.select(Some(Pattern::Black));
            }
            None => return false,
        }
        true
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        if let Some(pattern) = self.pattern {
            return display.clear(pattern.color());
        }
        display.clear(Gray4::WHITE)?;
        button(display, BACK_TARGET, "Back", false, true)?;
        let style = MonoTextStyle::new(&FONT_10X20, Gray4::BLACK);
        for (y, text) in [
            (58, "DIAGNOSTICS"),
            (150, "Display check"),
            (190, "Black, white, then gray. Full refreshes."),
            (230, "Each image stays until you advance."),
            (270, "Wait until the refresh stops."),
            (310, "Then press and release:"),
            (
                340,
                self.advance.map_or("Unavailable", |control| control.label),
            ),
            (540, "Exit the check:"),
            (574, self.exit_hint),
            (650, "USB inspection is available while"),
            (680, "a completed test image stays visible."),
            (
                750,
                if !self.available {
                    "Display unavailable. Restart required."
                } else if self.complete {
                    "Display check complete."
                } else {
                    ""
                },
            ),
        ] {
            Text::new(text, Point::new(28, y), style).draw(display)?;
        }
        button(
            display,
            START,
            "Start display check",
            false,
            self.available && self.advance.is_some(),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paper_mesh_platform::input::buttons::{ButtonCapability, ButtonId};

    fn start(check: &mut Diagnostics) {
        check.open();
        check.touch(TouchEvent::touch(0, START.center()));
        check.touch(TouchEvent::release(0, START.center()));
    }

    fn configured() -> Diagnostics {
        let mut check = Diagnostics::new();
        check.set_controls(
            ButtonDescriptor {
                id: ButtonId(7),
                label: "Next",
                capability: ButtonCapability::Edges,
            },
            "Press Exit.",
        );
        start(&mut check);
        check
    }

    #[test]
    fn stale_events_and_synchronized_holds_cannot_advance_a_completed_image() {
        let mut check = configured();
        check.display_completed(20);
        assert_eq!(check.held_pattern(), None);
        check.submitted();
        check.display_completed(100);
        assert_eq!(check.held_pattern(), Some(Pattern::Black));
        for (id, state, at_ms) in [
            (7, ButtonState::Pressed, 99),
            (7, ButtonState::Released, 101),
            (7, ButtonState::Synchronized { pressed: true }, 102),
            (7, ButtonState::Released, 103),
            (0, ButtonState::Pressed, 104),
            (0, ButtonState::Released, 105),
            (7, ButtonState::Pressed, 106),
            (7, ButtonState::Cancel, 107),
            (7, ButtonState::Released, 108),
        ] {
            assert!(!check.advance(ButtonEvent {
                id: ButtonId(id),
                state,
                at_ms
            }));
        }
        assert!(!check.advance(ButtonEvent {
            id: ButtonId(7),
            state: ButtonState::Pressed,
            at_ms: 109
        }));
        assert!(check.advance(ButtonEvent {
            id: ButtonId(7),
            state: ButtonState::Released,
            at_ms: 110
        }));
        assert_eq!(check.pattern(), Some(Pattern::White));
        assert_eq!(check.held_pattern(), None);
    }

    #[test]
    fn missing_controls_or_failed_display_cannot_start_a_check() {
        let mut missing = Diagnostics::new();
        start(&mut missing);
        assert_eq!(missing.pattern(), None);
        let mut failed = configured();
        failed.submitted();
        failed.display_completed(100);
        failed.display_failed();
        assert_eq!(failed.held_pattern(), None);
        start(&mut failed);
        assert_eq!(failed.pattern(), None);
        failed.touch(TouchEvent::touch(0, BACK_TARGET.center()));
        failed.touch(TouchEvent::release(0, BACK_TARGET.center()));
        assert!(!failed.is_open());
    }
}
