//! Power details, observation expiry, and application display policy.

use core::fmt::{self, Write};
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Alignment, Text},
};
use heapless::String;
use paper_mesh_platform::{
    TouchEvent,
    input::TapGesture,
    power::{
        BatteryState, ChargeFault, ChargeState, ChargerState, GaugeState, Power, PowerAvailability,
        PowerReading, PowerSnapshot, PowerSource, ThermistorState,
    },
    time::ObservationWindow,
    ui::{
        button,
        format::{measurement, temperature_celsius},
    },
};

use crate::BACK_TARGET;

pub const POWER_TARGET: Rectangle = Rectangle::new(Point::new(48, 620), Size::new(444, 64));
pub const BATTERY_TAB: Rectangle = Rectangle::new(Point::new(28, 104), Size::new(150, 56));
pub const CHARGER_TAB: Rectangle = Rectangle::new(Point::new(195, 104), Size::new(150, 56));
pub const SUSPEND_TAB: Rectangle = Rectangle::new(Point::new(362, 104), Size::new(150, 56));
pub const POWER_MAX_AGE_MS: u64 = 10_000;
pub const POWER_REFRESH_MS: u64 = 5_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadingStatus {
    Starting,
    Unavailable,
    Fault,
    NoData,
    Fresh,
    Stale,
}

impl ReadingStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Starting => "Startup",
            Self::Unavailable => "Unavailable",
            Self::Fault => "Read fault",
            Self::NoData => "No data",
            Self::Fresh => "Ready",
            Self::Stale => "Data expired",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PowerView {
    pub battery_status: ReadingStatus,
    pub charger_status: ReadingStatus,
    pub battery: Option<BatteryState>,
    pub charger: Option<ChargerState>,
    pub battery_age_seconds: Option<u64>,
    pub charger_age_seconds: Option<u64>,
}

impl PowerView {
    const INITIAL: Self = Self {
        battery_status: ReadingStatus::Starting,
        charger_status: ReadingStatus::Starting,
        battery: None,
        charger: None,
        battery_age_seconds: None,
        charger_age_seconds: None,
    };

    fn new(snapshot: PowerSnapshot, now_ms: u64) -> Self {
        let (battery_status, battery, battery_age_seconds) = reading(snapshot.battery, now_ms);
        let (charger_status, charger, charger_age_seconds) = reading(snapshot.charger, now_ms);
        Self {
            battery_status,
            charger_status,
            battery,
            charger,
            battery_age_seconds,
            charger_age_seconds,
        }
    }
}

fn reading<T: Copy>(
    value: PowerReading<T>,
    now_ms: u64,
) -> (ReadingStatus, Option<T>, Option<u64>) {
    let window = ObservationWindow::new(now_ms, POWER_MAX_AGE_MS);
    let age = value
        .observation
        .and_then(|observation| window.age_ms(observation.observed_at_ms));
    let status = match value.availability {
        PowerAvailability::Starting => ReadingStatus::Starting,
        PowerAvailability::Unavailable => ReadingStatus::Unavailable,
        PowerAvailability::Fault => ReadingStatus::Fault,
        PowerAvailability::Ready if value.observation.is_none() => ReadingStatus::NoData,
        PowerAvailability::Ready
            if !value
                .observation
                .is_some_and(|value| window.contains(value.observed_at_ms)) =>
        {
            ReadingStatus::Stale
        }
        PowerAvailability::Ready => ReadingStatus::Fresh,
    };
    (
        status,
        value
            .observation
            .filter(|_| status == ReadingStatus::Fresh)
            .map(|value| value.value),
        age.map(|value| value / 1000),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Suspend,
    Back,
    Battery,
    Charger,
}

pub struct PowerScreen {
    pub suspend: crate::suspend::SuspendScreen,
    open: bool,
    charger_tab: bool,
    gesture: TapGesture<Action>,
    view: PowerView,
    next_update_ms: u64,
    expires_at_ms: Option<u64>,
}

impl Default for PowerScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerScreen {
    pub const fn new() -> Self {
        Self {
            suspend: crate::suspend::SuspendScreen::new(),
            open: false,
            charger_tab: false,
            gesture: TapGesture::new(),
            view: PowerView::INITIAL,
            next_update_ms: 0,
            expires_at_ms: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn charger_tab(&self) -> bool {
        self.charger_tab
    }
    pub fn view(&self) -> PowerView {
        self.view
    }

    pub fn open(&mut self) {
        self.suspend.open = false;
        self.open = true;
        self.charger_tab = false;
        self.gesture.cancel();
        self.next_update_ms = 0;
    }

    pub fn next_deadline_ms(&self) -> Option<u64> {
        self.open.then_some(
            self.next_update_ms
                .min(self.expires_at_ms.unwrap_or(u64::MAX)),
        )
    }

    pub fn poll(&mut self, now_ms: u64, power: &mut impl Power) -> bool {
        let snapshot = power.snapshot(now_ms);
        let view = PowerView::new(snapshot, now_ms);
        self.expires_at_ms = ObservationWindow::new(now_ms, POWER_MAX_AGE_MS).next_expiry_ms(
            [
                snapshot
                    .battery
                    .observation
                    .map(|value| value.observed_at_ms),
                snapshot
                    .charger
                    .observation
                    .map(|value| value.observed_at_ms),
            ]
            .into_iter()
            .flatten(),
        );
        let urgent = view.battery_status != self.view.battery_status
            || view.charger_status != self.view.charger_status
            || view.charger != self.view.charger
            || battery_status(view.battery) != battery_status(self.view.battery);
        if !self.open || now_ms >= self.next_update_ms || urgent {
            let changed = view != self.view;
            self.view = view;
            self.next_update_ms = now_ms.saturating_add(POWER_REFRESH_MS);
            return self.open && changed;
        }
        false
    }

    pub fn touch(&mut self, event: TouchEvent) -> bool {
        if self.suspend.open {
            return self.suspend.touch(event);
        }
        let action = self.gesture.update(event, |point| {
            [
                (BACK_TARGET, Action::Back),
                (BATTERY_TAB, Action::Battery),
                (CHARGER_TAB, Action::Charger),
                (SUSPEND_TAB, Action::Suspend),
            ]
            .into_iter()
            .find_map(|(target, action)| target.contains(point).then_some(action))
        });
        match action {
            Some(Action::Suspend) => self.suspend.open = true,
            Some(Action::Back) => self.open = false,
            Some(Action::Battery) => self.charger_tab = false,
            Some(Action::Charger) => self.charger_tab = true,
            None => return false,
        }
        true
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        if self.suspend.open {
            return self.suspend.render(display);
        }
        display.clear(Gray4::WHITE)?;
        text(display, "POWER", 28, 58, false)?;
        button(display, BACK_TARGET, "Back", false, true)?;
        button(display, BATTERY_TAB, "Battery", !self.charger_tab, true)?;
        button(display, CHARGER_TAB, "Charger", self.charger_tab, true)?;
        button(display, SUSPEND_TAB, "Suspend", false, true)?;
        if self.charger_tab {
            self.render_charger(display)?;
        } else {
            self.render_battery(display)?;
        }
        row(
            display,
            906,
            "Battery data",
            format_args!(
                "{} / {} s",
                self.view.battery_status.label(),
                measurement(self.view.battery_age_seconds, "")
            ),
        )?;
        row(
            display,
            936,
            "Charger data",
            format_args!(
                "{} / {} s",
                self.view.charger_status.label(),
                measurement(self.view.charger_age_seconds, "")
            ),
        )
    }

    fn render_battery<D: DrawTarget<Color = Gray4>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let battery = self.view.battery;
        let soc = battery.and_then(|value| value.state_of_charge_percent);
        row(
            display,
            210,
            "State of charge",
            format_args!("{}", measurement(soc, "%")),
        )?;
        Rectangle::new(Point::new(28, 232), Size::new(484, 20))
            .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 1))
            .draw(display)?;
        if let Some(soc) = soc {
            Rectangle::new(
                Point::new(31, 235),
                Size::new(478 * u32::from(soc.min(100)) / 100, 14),
            )
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(5)))
            .draw(display)?;
        }
        row(
            display,
            286,
            "Charge phase",
            format_args!(
                "{}",
                self.view
                    .charger
                    .map_or("--", |value| charge_label(value.charge))
            ),
        )?;
        let b = battery.unwrap_or_default();
        let values = [
            (
                "Battery present",
                measurement(battery.map(|value| yes(value.present)), ""),
            ),
            ("Voltage", measurement(b.voltage_mv, "mV")),
            ("Current", measurement(b.current_ma, "mA")),
            ("Average current", measurement(b.average_current_ma, "mA")),
            ("Average power", measurement(b.average_power_mw, "mW")),
            (
                "Gauge temperature",
                temperature_celsius(b.gauge_temperature_centi_c),
            ),
            (
                "Remaining capacity",
                measurement(b.remaining_capacity_mah, "mAh"),
            ),
            ("Full capacity", measurement(b.full_capacity_mah, "mAh")),
            ("Design capacity", measurement(b.design_capacity_mah, "mAh")),
            (
                "State of health",
                measurement(b.state_of_health_percent, "%"),
            ),
            ("Cycles", measurement(b.cycles, "")),
            ("Time to empty", measurement(b.time_to_empty_minutes, "min")),
            ("Time to full", measurement(b.time_to_full_minutes, "min")),
            (
                "Gauge state",
                measurement(battery.map(|value| gauge_label(value.gauge)), ""),
            ),
        ];
        for (index, (name, value)) in values.iter().enumerate() {
            row(
                display,
                316 + index as i32 * 30,
                name,
                format_args!("{value}"),
            )?;
        }
        row(
            display,
            746,
            "Full / empty / discharge",
            format_args!(
                "{} / {} / {}",
                flag(battery.map(|b| b.fully_charged)),
                flag(battery.map(|b| b.fully_discharged)),
                flag(battery.map(|b| b.discharge_active))
            ),
        )?;
        row(
            display,
            778,
            "Hot / inhibit / shutdown",
            format_args!(
                "{} / {} / {}",
                flag(battery.map(|b| b.overtemperature)),
                flag(battery.map(|b| b.charge_inhibited)),
                flag(battery.map(|b| b.shutdown_requested))
            ),
        )?;
        text(
            display,
            "Positive current enters the battery.",
            28,
            826,
            true,
        )?;
        text(
            display,
            if battery.is_some_and(|b| b.estimates_verified) {
                "Cell profile and calibration verified."
            } else {
                "Cell profile and calibration unverified."
            },
            28,
            854,
            true,
        )
    }

    fn render_charger<D: DrawTarget<Color = Gray4>>(
        &self,
        display: &mut D,
    ) -> Result<(), D::Error> {
        let c = self.view.charger;
        let values = [
            (
                "Input class",
                measurement(c.map(|c| source_label(c.source)), ""),
            ),
            (
                "Charge phase",
                measurement(c.map(|c| charge_label(c.charge)), ""),
            ),
            ("Power good", measurement(c.map(|c| yes(c.power_good)), "")),
            (
                "Charge enabled",
                measurement(c.map(|c| yes(c.charge_enabled)), ""),
            ),
            (
                "Input suspended",
                measurement(c.map(|c| yes(c.input_suspended)), ""),
            ),
            (
                "Input current setting",
                measurement(c.map(|c| c.input_limit_ma), "mA"),
            ),
            (
                "Charge current setting",
                measurement(c.map(|c| c.charge_limit_ma), "mA"),
            ),
            (
                "Charge voltage setting",
                measurement(c.map(|c| c.charge_voltage_mv), "mV"),
            ),
            (
                "Termination setting",
                measurement(c.map(|c| c.termination_current_ma), "mA"),
            ),
            (
                "Input voltage limit",
                measurement(c.map(|c| c.input_voltage_limit_mv), "mV"),
            ),
        ];
        for (index, (name, value)) in values.iter().enumerate() {
            row(
                display,
                208 + index as i32 * 32,
                name,
                format_args!("{value}"),
            )?;
        }
        row(
            display,
            528,
            "Input limited: I / V",
            format_args!(
                "{} / {}",
                flag(c.map(|c| c.input_current_limited)),
                flag(c.map(|c| c.input_voltage_limited))
            ),
        )?;
        row(
            display,
            560,
            "Thermal / minimum SYS",
            format_args!(
                "{} / {}",
                flag(c.map(|c| c.thermal_regulation)),
                flag(c.map(|c| c.minimum_system_voltage))
            ),
        )?;
        text(display, "Fault", 28, 614, true)?;
        text(display, "Current", 188, 614, true)?;
        text(display, "Latched", 368, 614, true)?;
        let fault_rows = [
            (
                "Charge",
                c.map(|c| fault_label(c.faults.charge)),
                c.map(|c| fault_label(c.latched_faults.charge)),
            ),
            (
                "Thermistor",
                c.map(|c| thermistor_label(c.faults.thermistor)),
                c.map(|c| thermistor_label(c.latched_faults.thermistor)),
            ),
            (
                "Battery OV",
                c.map(|c| yes(c.faults.battery_overvoltage)),
                c.map(|c| yes(c.latched_faults.battery_overvoltage)),
            ),
            (
                "Boost",
                c.map(|c| yes(c.faults.boost)),
                c.map(|c| yes(c.latched_faults.boost)),
            ),
            (
                "Watchdog",
                c.map(|c| yes(c.faults.watchdog_expired)),
                c.map(|c| yes(c.latched_faults.watchdog_expired)),
            ),
        ];
        for (index, (name, current, latched)) in fault_rows.iter().enumerate() {
            let y = 646 + index as i32 * 32;
            text(display, name, 28, y, true)?;
            text(display, current.unwrap_or("--"), 188, y, false)?;
            text(display, latched.unwrap_or("--"), 368, y, false)?;
        }
        text(
            display,
            "Settings are not measured currents.",
            28,
            826,
            true,
        )?;
        text(
            display,
            "Latched: since the previous device read.",
            28,
            854,
            true,
        )
    }
}

fn battery_status(b: Option<BatteryState>) -> Option<(GaugeState, u16, u16)> {
    fn bits(values: impl IntoIterator<Item = bool>) -> u16 {
        values
            .into_iter()
            .enumerate()
            .fold(0, |mask, (index, value)| mask | (u16::from(value) << index))
    }
    b.map(|b| {
        (
            b.gauge,
            bits([
                b.present,
                b.fully_charged,
                b.fully_discharged,
                b.discharge_active,
                b.charge_inhibited,
                b.overtemperature,
                b.shutdown_requested,
                b.estimates_verified,
            ]),
            bits([
                b.state_of_charge_percent.is_some(),
                b.state_of_health_percent.is_some(),
                b.voltage_mv.is_some(),
                b.current_ma.is_some(),
                b.average_current_ma.is_some(),
                b.average_power_mw.is_some(),
                b.gauge_temperature_centi_c.is_some(),
                b.remaining_capacity_mah.is_some(),
                b.full_capacity_mah.is_some(),
                b.design_capacity_mah.is_some(),
                b.cycles.is_some(),
                b.time_to_empty_minutes.is_some(),
                b.time_to_full_minutes.is_some(),
            ]),
        )
    })
}

fn yes(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}
fn flag(value: Option<bool>) -> &'static str {
    value.map_or("--", yes)
}
pub fn charge_label(value: ChargeState) -> &'static str {
    match value {
        ChargeState::Idle => "Idle",
        ChargeState::Precharge => "Precharge",
        ChargeState::Fast => "Fast charge",
        ChargeState::Complete => "Complete",
    }
}
pub fn source_label(value: PowerSource) -> &'static str {
    match value {
        PowerSource::None => "No input",
        PowerSource::Usb => "USB class",
        PowerSource::Adapter => "Adapter class",
        PowerSource::Output => "OTG output",
        PowerSource::Unknown => "Unknown",
    }
}
fn gauge_label(value: GaugeState) -> &'static str {
    match value {
        GaugeState::Initializing => "Startup",
        GaugeState::Ready => "Ready",
        GaugeState::Configuring => "Configuration",
        GaugeState::Calibrating => "Calibration",
    }
}
fn fault_label(value: ChargeFault) -> &'static str {
    match value {
        ChargeFault::None => "None",
        ChargeFault::Input => "Input",
        ChargeFault::ThermalShutdown => "Thermal stop",
        ChargeFault::SafetyTimer => "Safety timer",
    }
}
fn thermistor_label(value: ThermistorState) -> &'static str {
    match value {
        ThermistorState::Normal => "Normal",
        ThermistorState::Warm => "Warm",
        ThermistorState::Cool => "Cool",
        ThermistorState::Cold => "Cold",
        ThermistorState::Hot => "Hot",
        ThermistorState::Unknown => "Unknown",
    }
}
fn text<D: DrawTarget<Color = Gray4>>(
    display: &mut D,
    value: &str,
    x: i32,
    y: i32,
    muted: bool,
) -> Result<(), D::Error> {
    Text::new(
        value,
        Point::new(x, y),
        MonoTextStyle::new(
            &FONT_10X20,
            if muted { Gray4::new(6) } else { Gray4::BLACK },
        ),
    )
    .draw(display)?;
    Ok(())
}
fn row<D: DrawTarget<Color = Gray4>>(
    display: &mut D,
    y: i32,
    name: &str,
    value: fmt::Arguments<'_>,
) -> Result<(), D::Error> {
    text(display, name, 28, y, true)?;
    let mut label = String::<64>::new();
    let _ = label.write_fmt(value);
    Text::with_alignment(
        &label,
        Point::new(512, y),
        MonoTextStyle::new(&FONT_10X20, Gray4::BLACK),
        Alignment::Right,
    )
    .draw(display)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Source(PowerSnapshot);
    impl Power for Source {
        fn snapshot(&mut self, _: u64) -> PowerSnapshot {
            self.0
        }
    }

    fn source(at_ms: u64) -> Source {
        Source(PowerSnapshot {
            battery: PowerReading::ready(
                BatteryState {
                    present: true,
                    gauge: GaugeState::Ready,
                    voltage_mv: Some(3800),
                    state_of_charge_percent: Some(60),
                    ..BatteryState::default()
                },
                at_ms,
            ),
            ..PowerSnapshot::INITIAL
        })
    }

    #[test]
    fn future_faulted_and_expired_readings_cannot_retain_measurements() {
        let mut source = source(1000);
        let mut screen = PowerScreen::new();
        screen.open();
        screen.poll(999, &mut source);
        assert_eq!(screen.view.battery_status, ReadingStatus::Stale);
        assert_eq!(screen.view.battery, None);
        screen.poll(1000, &mut source);
        assert!(screen.view.battery.is_some());
        screen.poll(11_000, &mut source);
        assert_eq!(screen.view.battery, None);
        assert_eq!(screen.view.battery_age_seconds, Some(10));
        source.0.battery.availability = PowerAvailability::Fault;
        screen.poll(1001, &mut source);
        assert_eq!(screen.view.battery_status, ReadingStatus::Fault);
        assert_eq!(screen.view.battery, None);
    }

    #[test]
    fn measurements_wait_for_display_interval_but_validity_changes_do_not() {
        let mut source = source(0);
        let mut screen = PowerScreen::new();
        screen.open();
        assert!(screen.poll(0, &mut source));
        source
            .0
            .battery
            .observation
            .as_mut()
            .unwrap()
            .value
            .voltage_mv = Some(3900);
        assert!(!screen.poll(1000, &mut source));
        assert_eq!(screen.view.battery.unwrap().voltage_mv, Some(3800));
        assert!(screen.poll(5000, &mut source));
        assert_eq!(screen.view.battery.unwrap().voltage_mv, Some(3900));
        source
            .0
            .battery
            .observation
            .as_mut()
            .unwrap()
            .value
            .voltage_mv = None;
        assert!(screen.poll(5001, &mut source));
        assert_eq!(screen.view.battery.unwrap().voltage_mv, None);
        assert_eq!(screen.next_deadline_ms(), Some(10_000));
    }
}
