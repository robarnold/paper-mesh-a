//! Shared discovery controls and grayscale rendering.

use crate::BACK_TARGET;
use core::fmt::Write;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::Text,
};
use heapless::String;
use paper_mesh_platform::{
    TouchEvent,
    discovery::{
        AddressKind, Discovery, RadioKind, ScanError, ScanRequest, ScanResults, ScanSource,
    },
    input::TapGesture,
    ui::button,
};

pub const SCAN_TIMEOUT_MS: u64 = 10_000;

pub const DEBUGGER_TARGET: Rectangle = Rectangle::new(Point::new(48, 190), Size::new(444, 64));
pub const WIFI_TARGET: Rectangle = Rectangle::new(Point::new(28, 112), Size::new(228, 60));
pub const BLUETOOTH_TARGET: Rectangle = Rectangle::new(Point::new(268, 112), Size::new(244, 60));
pub const SCAN_TARGET: Rectangle = Rectangle::new(Point::new(28, 808), Size::new(484, 60));
pub const PREVIOUS_TARGET: Rectangle = Rectangle::new(Point::new(28, 884), Size::new(140, 52));
pub const NEXT_TARGET: Rectangle = Rectangle::new(Point::new(372, 884), Size::new(140, 52));
const PAGE_SIZE: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanState {
    Idle,
    Requested,
    Active,
    Complete,
    Error(ScanError),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Back,
    Wifi,
    Bluetooth,
    Scan,
    Previous,
    Next,
}

pub struct Debugger {
    open: bool,
    kind: RadioKind,
    state: ScanState,
    source: Option<ScanSource>,
    results: ScanResults,
    observed_at_ms: Option<u64>,
    active: Option<ScanRequest>,
    request_id: u64,
    page: usize,
    gesture: TapGesture<Action>,
}

impl Default for Debugger {
    fn default() -> Self {
        Self::new()
    }
}

impl Debugger {
    pub const fn new() -> Self {
        Self {
            open: false,
            kind: RadioKind::Wifi,
            state: ScanState::Idle,
            source: None,
            results: ScanResults::new(),
            observed_at_ms: None,
            active: None,
            request_id: 0,
            page: 0,
            gesture: TapGesture::new(),
        }
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn kind(&self) -> RadioKind {
        self.kind
    }
    pub fn state(&self) -> ScanState {
        self.state
    }
    pub fn source(&self) -> Option<ScanSource> {
        self.source
    }
    pub fn results(&self) -> &ScanResults {
        &self.results
    }
    pub fn observed_at_ms(&self) -> Option<u64> {
        self.observed_at_ms
    }
    pub fn page(&self) -> usize {
        self.page
    }

    pub fn open(&mut self) {
        self.open = true;
        self.gesture.cancel();
        self.request_scan();
    }

    fn request_scan(&mut self) {
        self.state = ScanState::Requested;
        self.results = ScanResults::default();
        self.observed_at_ms = None;
        self.source = None;
        self.page = 0;
    }

    pub fn next_deadline_ms(&self) -> Option<u64> {
        self.active.map(|request| request.deadline_ms)
    }

    /// This method starts requests and accepts a completion for the active request before its deadline.
    /// It returns true when the visible state changes.
    pub fn poll(&mut self, now_ms: u64, discovery: &mut impl Discovery) -> bool {
        if !self.open || self.state == ScanState::Requested {
            if let Some(request) = self.active.take() {
                discovery.cancel(request);
            }
            if !self.open {
                return false;
            }
            self.request_id += 1;
            let request = ScanRequest {
                id: self.request_id,
                kind: self.kind,
                deadline_ms: now_ms.saturating_add(SCAN_TIMEOUT_MS),
            };
            match discovery.start(request, now_ms) {
                Ok(source) => {
                    self.source = Some(source);
                    self.active = Some(request);
                    self.state = ScanState::Active;
                }
                Err(error) => self.state = ScanState::Error(error),
            }
            return true;
        }
        if let Some(request) = self.active {
            if let Some(completion) = discovery.poll(now_ms, &mut self.results) {
                if completion.request == request
                    && completion.observed_at_ms <= now_ms
                    && completion.observed_at_ms <= request.deadline_ms
                {
                    self.active = None;
                    match completion.result {
                        Ok(()) => {
                            self.observed_at_ms = Some(completion.observed_at_ms);
                            self.state = ScanState::Complete;
                        }
                        Err(error) => {
                            self.results.clear();
                            self.state = ScanState::Error(error);
                        }
                    }
                    return true;
                }
                self.results.clear();
            }
            if now_ms >= request.deadline_ms {
                discovery.cancel(request);
                self.active = None;
                self.state = ScanState::Error(ScanError::Timeout);
                return true;
            }
        }
        false
    }

    fn action(point: Point) -> Option<Action> {
        [
            (BACK_TARGET, Action::Back),
            (WIFI_TARGET, Action::Wifi),
            (BLUETOOTH_TARGET, Action::Bluetooth),
            (SCAN_TARGET, Action::Scan),
            (PREVIOUS_TARGET, Action::Previous),
            (NEXT_TARGET, Action::Next),
        ]
        .into_iter()
        .find_map(|(target, action)| target.contains(point).then_some(action))
    }

    pub fn touch(&mut self, event: TouchEvent) -> bool {
        let Some(action) = self.gesture.update(event, Self::action) else {
            return false;
        };
        match action {
            Action::Back => self.open = false,
            Action::Wifi | Action::Bluetooth => {
                let kind = if action == Action::Wifi {
                    RadioKind::Wifi
                } else {
                    RadioKind::BluetoothLe
                };
                if self.kind == kind {
                    return false;
                }
                self.kind = kind;
                self.request_scan();
            }
            Action::Scan => {
                if matches!(self.state, ScanState::Requested | ScanState::Active) {
                    return false;
                }
                self.request_scan();
            }
            Action::Previous => {
                if self.page == 0 {
                    return false;
                }
                self.page -= 1;
            }
            Action::Next => {
                if (self.page + 1) * PAGE_SIZE >= self.results.networks().len() {
                    return false;
                }
                self.page += 1;
            }
        }
        true
    }

    pub fn render<D: DrawTarget<Color = Gray4>>(&self, display: &mut D) -> Result<(), D::Error> {
        display.clear(Gray4::WHITE)?;
        let black = MonoTextStyle::new(&FONT_10X20, Gray4::BLACK);
        let muted = MonoTextStyle::new(&FONT_10X20, Gray4::new(6));
        Text::new("RADIO DEBUGGER", Point::new(28, 58), black).draw(display)?;
        button(display, BACK_TARGET, "Back", false, true)?;
        button(
            display,
            WIFI_TARGET,
            "Wi-Fi",
            self.kind == RadioKind::Wifi,
            true,
        )?;
        button(
            display,
            BLUETOOTH_TARGET,
            "Bluetooth LE",
            self.kind == RadioKind::BluetoothLe,
            true,
        )?;
        let source = match self.source {
            Some(ScanSource::Simulated) => "SIMULATED DATA",
            Some(ScanSource::Device) => "DEVICE DATA",
            None => "NO SCAN DATA",
        };
        Text::new(source, Point::new(28, 208), black).draw(display)?;
        let status = match self.state {
            ScanState::Idle | ScanState::Requested => "Scan requested",
            ScanState::Active => "Scan in progress...",
            ScanState::Complete if self.results.networks().is_empty() => {
                if self.kind == RadioKind::Wifi {
                    "No networks found"
                } else {
                    "No devices found"
                }
            }
            ScanState::Complete => "Strongest signal first / RSSI in dBm",
            ScanState::Error(ScanError::Unsupported) => "Scan adapter unavailable",
            ScanState::Error(ScanError::Busy) => "Radio busy. Try again.",
            ScanState::Error(ScanError::Failed) => "Scan failed. Try again.",
            ScanState::Error(ScanError::Timeout) => "Scan timed out. Try again.",
        };
        Text::new(status, Point::new(28, 240), muted).draw(display)?;
        let mut label = String::<64>::new();
        if let Some(time) = self.observed_at_ms {
            let _ = write!(
                label,
                "Scan time: {} s / {} results{}",
                time / 1000,
                self.results.networks().len(),
                if self.results.truncated {
                    " (limit)"
                } else {
                    ""
                }
            );
            Text::new(&label, Point::new(28, 270), muted).draw(display)?;
        }
        for (index, network) in self
            .results
            .networks()
            .iter()
            .skip(self.page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .enumerate()
        {
            let y = 310 + index as i32 * 96;
            let name = if network.name.is_empty() {
                if self.kind == RadioKind::Wifi {
                    "Hidden network"
                } else {
                    "Unnamed device"
                }
            } else {
                network.name.as_str()
            };
            Text::new(name, Point::new(28, y), black).draw(display)?;
            label.clear();
            let [a, b, c, d, e, f] = network.address;
            let _ = write!(label, "{a:02X}:{b:02X}:{c:02X}:{d:02X}:{e:02X}:{f:02X}");
            if self.kind == RadioKind::BluetoothLe {
                let _ = label.push_str(if network.address_kind == AddressKind::Random {
                    " random"
                } else {
                    " public"
                });
            }
            Text::new(&label, Point::new(28, y + 26), muted).draw(display)?;
            label.clear();
            if let Some(rssi) = network.rssi_dbm {
                let _ = write!(label, "{rssi} dBm");
            } else {
                let _ = label.push_str("RSSI unknown");
            }
            Text::new(&label, Point::new(28, y + 52), black).draw(display)?;
            Rectangle::new(Point::new(28, y + 68), Size::new(484, 1))
                .into_styled(PrimitiveStyle::with_fill(Gray4::new(12)))
                .draw(display)?;
        }
        let active = matches!(self.state, ScanState::Active | ScanState::Requested);
        button(
            display,
            SCAN_TARGET,
            if active {
                "Scan in progress..."
            } else {
                "Scan again"
            },
            true,
            !active,
        )?;
        button(display, PREVIOUS_TARGET, "Previous", false, self.page > 0)?;
        button(
            display,
            NEXT_TARGET,
            "Next",
            false,
            (self.page + 1) * PAGE_SIZE < self.results.networks().len(),
        )?;
        label.clear();
        let pages = self.results.networks().len().div_ceil(PAGE_SIZE).max(1);
        let _ = write!(label, "{} / {}", self.page + 1, pages);
        Text::new(&label, Point::new(236, 918), black).draw(display)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paper_mesh_platform::discovery::{ScanCompletion, UnavailableDiscovery};

    #[derive(Default)]
    struct Adapter {
        request: Option<ScanRequest>,
        completion: Option<ScanCompletion>,
        canceled: Option<ScanRequest>,
    }

    impl Discovery for Adapter {
        fn start(&mut self, request: ScanRequest, _: u64) -> Result<ScanSource, ScanError> {
            self.request = Some(request);
            Ok(ScanSource::Device)
        }
        fn poll(&mut self, _: u64, results: &mut ScanResults) -> Option<ScanCompletion> {
            let completion = self.completion.take()?;
            results.clear();
            Some(completion)
        }
        fn cancel(&mut self, request: ScanRequest) {
            self.canceled = Some(request);
        }
    }

    fn press(debugger: &mut Debugger, target: Rectangle) {
        debugger.touch(TouchEvent::Down(target.center()));
        debugger.touch(TouchEvent::Up(target.center()));
    }

    #[test]
    fn ignores_a_canceled_request_and_times_out_without_a_completion() {
        let mut debugger = Debugger::default();
        let mut adapter = Adapter::default();
        debugger.open();
        debugger.poll(100, &mut adapter);
        let old = adapter.request.unwrap();
        press(&mut debugger, BLUETOOTH_TARGET);
        debugger.poll(200, &mut adapter);
        assert_eq!(adapter.canceled, Some(old));
        let current = adapter.request.unwrap();
        assert_ne!(old.id, current.id);
        adapter.completion = Some(ScanCompletion {
            request: old,
            observed_at_ms: 300,
            result: Ok(()),
        });
        assert!(!debugger.poll(300, &mut adapter));
        assert_eq!(debugger.state(), ScanState::Active);
        debugger.poll(current.deadline_ms, &mut adapter);
        assert_eq!(debugger.state(), ScanState::Error(ScanError::Timeout));
        assert_eq!(adapter.canceled, Some(current));
        assert_eq!(debugger.next_deadline_ms(), None);
    }

    #[test]
    fn unavailable_adapter_and_incomplete_gestures_do_not_create_results() {
        let mut debugger = Debugger::default();
        debugger.open();
        debugger.poll(0, &mut UnavailableDiscovery);
        assert_eq!(debugger.state(), ScanState::Error(ScanError::Unsupported));
        assert_eq!(debugger.source(), None);
        debugger.touch(TouchEvent::Up(BLUETOOTH_TARGET.center()));
        debugger.touch(TouchEvent::Down(BLUETOOTH_TARGET.center()));
        debugger.touch(TouchEvent::Cancel);
        debugger.touch(TouchEvent::Up(BLUETOOTH_TARGET.center()));
        debugger.touch(TouchEvent::Down(BLUETOOTH_TARGET.center()));
        debugger.touch(TouchEvent::Up(WIFI_TARGET.center()));
        assert_eq!(debugger.kind(), RadioKind::Wifi);
        assert!(debugger.results().networks().is_empty());
    }
}
