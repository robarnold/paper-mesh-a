//! Suspend experiment controls and the most recent charge comparison.

use crate::BACK_TARGET;
use core::fmt::Write;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Gray4,
    prelude::*,
    primitives::Rectangle,
    text::Text,
};
use heapless::String;
use paper_mesh_platform::{
    TouchEvent,
    input::{TapGesture, buttons::ButtonDescriptor},
    suspend::{SuspendError, SuspendReport, SuspendRequest, WakeReason},
    ui::{button, format::measurement},
};

pub const ONE_MINUTE: Rectangle = Rectangle::new(Point::new(28, 160), Size::new(150, 56));
pub const TEN_MINUTES: Rectangle = Rectangle::new(Point::new(195, 160), Size::new(150, 56));
pub const ONE_HOUR: Rectangle = Rectangle::new(Point::new(362, 160), Size::new(150, 56));
pub const START: Rectangle = Rectangle::new(Point::new(28, 250), Size::new(484, 64));
pub const TOUCH_WAKE: Rectangle = Rectangle::new(Point::new(28, 326), Size::new(484, 56));

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Back,
    Duration(u64),
    Start,
    TouchWake,
}

pub struct SuspendScreen {
    pub open: bool,
    duration_ms: u64,
    pending: bool,
    request: Option<SuspendRequest>,
    result: Option<Result<SuspendReport, SuspendError>>,
    wake_controls: &'static [ButtonDescriptor],
    touch_wake: bool,
    gesture: TapGesture<Action>,
}

impl SuspendScreen {
    pub const fn new() -> Self {
        Self {
            open: false,
            duration_ms: 600_000,
            pending: false,
            request: None,
            result: None,
            wake_controls: &[],
            touch_wake: false,
            gesture: TapGesture::new(),
        }
    }
    pub fn set_controls(&mut self, controls: &'static [ButtonDescriptor]) {
        self.wake_controls = controls;
    }
    pub fn touch_wake(&self) -> bool {
        self.touch_wake
    }
    pub fn wake_label(&self, reason: WakeReason) -> &'static str {
        match reason {
            WakeReason::Timer => "Timer",
            WakeReason::Control(id) => self
                .wake_controls
                .iter()
                .find(|control| control.id == id)
                .map_or("Control", |control| control.label),
            WakeReason::TouchInterrupt => "Touch/Home IRQ",
            WakeReason::UnidentifiedInput => "Control IRQ",
        }
    }
    pub fn take_request(&mut self) -> Option<SuspendRequest> {
        self.request.take()
    }
    pub fn finish(&mut self, result: Result<SuspendReport, SuspendError>) {
        self.pending = false;
        self.result = Some(result);
        self.gesture.cancel();
    }
    pub fn result(&self) -> Option<Result<SuspendReport, SuspendError>> {
        self.result
    }
    pub fn touch(&mut self, event: TouchEvent) -> bool {
        if self.pending {
            return false;
        }
        let action = self.gesture.update(event, |point| {
            [
                (BACK_TARGET, Action::Back),
                (ONE_MINUTE, Action::Duration(60_000)),
                (TEN_MINUTES, Action::Duration(600_000)),
                (ONE_HOUR, Action::Duration(3_600_000)),
                (START, Action::Start),
                (TOUCH_WAKE, Action::TouchWake),
            ]
            .into_iter()
            .find_map(|(target, action)| target.contains(point).then_some(action))
        });
        match action {
            Some(Action::Back) => self.open = false,
            Some(Action::Duration(value)) => self.duration_ms = value,
            Some(Action::TouchWake) => self.touch_wake = !self.touch_wake,
            Some(Action::Start) => {
                self.pending = true;
                self.result = None;
                self.request = Some(SuspendRequest {
                    duration_ms: self.duration_ms,
                    touch_wake: self.touch_wake,
                });
            }
            None => return false,
        }
        true
    }
    pub fn render<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        display.clear(Gray4::WHITE)?;
        line(display, 58, format_args!("SUSPEND"))?;
        button(display, BACK_TARGET, "Back", false, !self.pending)?;
        line(display, 124, format_args!("Automatic wake after:"))?;
        for (target, label, duration) in [
            (ONE_MINUTE, "1 minute", 60_000),
            (TEN_MINUTES, "10 minutes", 600_000),
            (ONE_HOUR, "1 hour", 3_600_000),
        ] {
            button(
                display,
                target,
                label,
                self.duration_ms == duration,
                !self.pending,
            )?;
        }
        button(
            display,
            START,
            if self.pending {
                "Suspend requested"
            } else {
                "Start light sleep"
            },
            false,
            !self.pending,
        )?;
        button(
            display,
            TOUCH_WAKE,
            if self.touch_wake {
                "Touch/Home wake: on"
            } else {
                "Touch/Home wake: off"
            },
            self.touch_wake,
            !self.pending,
        )?;
        let mut controls = String::<48>::new();
        for control in self.wake_controls {
            if !controls.is_empty() {
                let _ = controls.push_str(" or ");
            }
            let _ = controls.push_str(control.label);
        }
        line(
            display,
            414,
            format_args!("Press {} to wake earlier.", controls),
        )?;
        line(
            display,
            446,
            format_args!("USB disconnects during suspend."),
        )?;
        match self.result {
            Some(Ok(report)) => {
                line(
                    display,
                    492,
                    format_args!(
                        "Resumed after {}.{:03} s",
                        report.elapsed_ms / 1000,
                        report.elapsed_ms % 1000
                    ),
                )?;
                let mut causes = String::<64>::new();
                for reason in report.wake_reasons.into_iter().flatten() {
                    if !causes.is_empty() {
                        let _ = causes.push_str(", ");
                    }
                    let _ = causes.push_str(self.wake_label(reason));
                }
                line(display, 526, format_args!("Wake: {}", causes))?;
                let capacity = |snapshot: paper_mesh_platform::power::PowerSnapshot| {
                    snapshot
                        .battery
                        .observation
                        .and_then(|value| value.value.remaining_capacity_mah)
                };
                line(
                    display,
                    566,
                    format_args!("Before: {}", measurement(capacity(report.before), "mAh")),
                )?;
                line(
                    display,
                    600,
                    format_args!("After:  {}", measurement(capacity(report.after), "mAh")),
                )?;
                line(
                    display,
                    634,
                    format_args!("Net loss: {}", measurement(report.charge_loss_mah(), "mAh")),
                )?;
                line(
                    display,
                    668,
                    format_args!(
                        "External power at endpoints: {}",
                        match report.external_power_at_endpoints() {
                            Some(true) => "yes",
                            Some(false) => "no",
                            None => "unknown",
                        }
                    ),
                )?;
            }
            Some(Err(error)) => line(
                display,
                492,
                format_args!(
                    "Suspend failed: {}",
                    match error {
                        SuspendError::Busy => "device busy",
                        SuspendError::Preparation => "preparation",
                        SuspendError::WakeActive => "release wake control",
                        SuspendError::WakeLocked => "driver wake lock",
                        SuspendError::Unsupported => "input unavailable",
                    }
                ),
            )?,
            None => {}
        }
        line(display, 730, format_args!("Gauge estimate; 1 mAh steps."))?;
        line(
            display,
            764,
            format_args!("Cell profile and accuracy unverified."),
        )?;
        line(
            display,
            798,
            format_args!("Zero change can be below resolution."),
        )?;
        line(
            display,
            832,
            format_args!("Negative loss means net charge gain."),
        )?;
        line(
            display,
            866,
            format_args!("Results remain until reset or next test."),
        )
    }
}

impl Default for SuspendScreen {
    fn default() -> Self {
        Self::new()
    }
}

fn line<D: DrawTarget<Color = Gray4>>(
    display: &mut D,
    y: i32,
    args: core::fmt::Arguments<'_>,
) -> Result<(), D::Error> {
    let mut text = String::<96>::new();
    let _ = text.write_fmt(args);
    Text::new(
        &text,
        Point::new(28, y),
        MonoTextStyle::new(&FONT_10X20, Gray4::BLACK),
    )
    .draw(display)?;
    Ok(())
}
