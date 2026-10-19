//! Native adapters and deterministic simulation.
//! The binary controls HTTP and wall-clock pacing.
//! Scenarios can use this library without either service.

use embedded_graphics::{pixelcolor::Gray4, prelude::*};
use paper_mesh_app::App;
use paper_mesh_platform::{
    Clock, Display, TouchEvent,
    rotation::{PortraitDisplay, PortraitRotation},
};
use serde::Serialize;
pub mod discovery;
pub mod location;
use discovery::{ScanScenario, SimDiscovery};
use location::{LocationScenario, SimLocation};
use paper_mesh_app::debugger::Debugger;
use paper_mesh_app::location::LocationScreen;
use paper_mesh_platform::discovery::{RadioKind, ScanSource};
use paper_mesh_platform::ui::format::satellite_label;
use paper_mesh_platform::{
    location::{Location, LocationAvailability},
    time::{ObservationWindow, UtcDateTime, UtcReference},
};

// This artificial delay exercises asynchronous refresh.
// It is not a panel measurement.
pub const REFRESH_MS: u64 = 250;
/// Native display dimensions for the current simulator profile.
pub const NATIVE_SCREEN_SIZE: Size = Size::new(960, 540);
pub const SCREEN_SIZE: Size = ROTATION.size();
const ROTATION: PortraitRotation = PortraitRotation::new(NATIVE_SCREEN_SIZE);

#[derive(Default)]
pub struct VirtualClock {
    now_ms: u64,
    utc: Option<UtcReference>,
}

impl Clock for VirtualClock {
    fn now_ms(&self) -> u64 {
        self.now_ms
    }

    fn now_utc(&self) -> Option<UtcDateTime> {
        self.utc?.at(self.now_ms)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayError {
    Busy,
}

pub struct SimDisplay {
    drawing: Vec<u8>,
    visible: Vec<u8>,
    pending: Option<Vec<u8>>,
    now_ms: u64,
    ready_at_ms: u64,
    completed_refreshes: u64,
}

impl Default for SimDisplay {
    fn default() -> Self {
        let pixels = (NATIVE_SCREEN_SIZE.width * NATIVE_SCREEN_SIZE.height) as usize;
        Self {
            drawing: vec![255; pixels],
            visible: vec![255; pixels],
            pending: None,
            now_ms: 0,
            ready_at_ms: 0,
            completed_refreshes: 0,
        }
    }
}

impl SimDisplay {
    fn advance_to(&mut self, now_ms: u64) {
        self.now_ms = now_ms;
        if self.is_busy() && now_ms >= self.ready_at_ms {
            self.visible = self
                .pending
                .take()
                .expect("busy display has a pending frame");
            self.completed_refreshes += 1;
        }
    }

    /// Completed frame in native row-major order (960 pixels per row).
    pub fn pixels(&self) -> &[u8] {
        &self.visible
    }

    pub fn native_png(&self) -> Result<Vec<u8>, png::EncodingError> {
        encode_png(NATIVE_SCREEN_SIZE, &self.visible)
    }
}

impl OriginDimensions for SimDisplay {
    fn size(&self) -> Size {
        NATIVE_SCREEN_SIZE
    }
}

impl DrawTarget for SimDisplay {
    type Color = Gray4;
    type Error = DisplayError;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        if self.is_busy() {
            return Err(DisplayError::Busy);
        }
        for Pixel(point, color) in pixels {
            if self.bounding_box().contains(point) {
                let index = point.y as usize * NATIVE_SCREEN_SIZE.width as usize + point.x as usize;
                self.drawing[index] = color.luma() * 17;
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        if self.is_busy() {
            return Err(DisplayError::Busy);
        }
        self.drawing.fill(color.luma() * 17);
        Ok(())
    }
}

impl Display for SimDisplay {
    fn is_busy(&self) -> bool {
        self.pending.is_some()
    }

    fn refresh(&mut self) -> Result<(), Self::Error> {
        if self.is_busy() {
            return Err(DisplayError::Busy);
        }
        self.pending = Some(self.drawing.clone());
        self.ready_at_ms = self.now_ms.saturating_add(REFRESH_MS);
        Ok(())
    }
}

fn encode_png(size: Size, pixels: &[u8]) -> Result<Vec<u8>, png::EncodingError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, size.width, size.height);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(pixels)?;
    }
    Ok(bytes)
}

#[derive(Serialize)]
pub struct Snapshot {
    pub now_ms: u64,
    pub utc_datetime: Option<String>,
    pub taps: u32,
    pub heartbeats: u64,
    pub last_heartbeat_seconds: Option<u64>,
    pub next_heartbeat_ms: u64,
    pub display_busy: bool,
    pub completed_refreshes: u64,
    pub debugger_open: bool,
    pub scan_kind: &'static str,
    pub scan_state: &'static str,
    pub scan_source: Option<&'static str>,
    pub scan_results: usize,
    pub scan_observed_at_ms: Option<u64>,
    pub scan_scenario: ScanScenario,
    pub location_open: bool,
    pub location_status: &'static str,
    pub latitude_microdegrees: Option<i32>,
    pub longitude_microdegrees: Option<i32>,
    pub elevation_mm: Option<i32>,
    pub speed_mm_s: Option<u32>,
    pub course_millidegrees: Option<u32>,
    pub utc_time: Option<[u8; 3]>,
    pub satellites_used: Option<u8>,
    pub location_age_seconds: Option<u64>,
    pub receiver_status: &'static str,
    pub receiver_age_seconds: Option<u64>,
    pub receiver_messages: u32,
    pub satellites_in_view: Option<u16>,
    pub satellites_with_signal: Option<u16>,
    pub satellite_age_seconds: Option<u64>,
    pub satellite_signals: Vec<SatelliteSnapshot>,
    pub location_scenario: LocationScenario,
    pub controls: Vec<Control>,
}

#[derive(Serialize)]
pub struct SatelliteSnapshot {
    pub system: &'static str,
    pub id: u16,
    pub cn0_db_hz: Option<u8>,
}

#[derive(Serialize)]
pub struct Control {
    label: &'static str,
    x: i32,
    y: i32,
}

pub struct Simulation {
    app: App,
    clock: VirtualClock,
    display: SimDisplay,
    discovery: SimDiscovery,
    location: SimLocation,
    heartbeats: u64,
    last_heartbeat_seconds: Option<u64>,
}

impl Default for Simulation {
    fn default() -> Self {
        let clock = VirtualClock::default();
        let mut simulation = Self {
            app: App::new(&clock),
            clock,
            display: SimDisplay::default(),
            discovery: SimDiscovery::default(),
            location: SimLocation::default(),
            heartbeats: 0,
            last_heartbeat_seconds: None,
        };
        simulation.poll();
        simulation
    }
}

impl Simulation {
    fn poll(&mut self) {
        self.display.advance_to(self.clock.now_ms());
        let location = self.location.snapshot(self.clock.now_ms());
        if location.availability == LocationAvailability::Ready
            && let Some(reference) = location.utc.and_then(|utc| utc.reference())
            && ObservationWindow::new(self.clock.now_ms(), 2_000).contains(reference.observed_at_ms)
            && self
                .clock
                .utc
                .is_none_or(|previous| reference.observed_at_ms > previous.observed_at_ms)
        {
            self.clock.utc = Some(reference);
        }
        self.app.poll_discovery(&self.clock, &mut self.discovery);
        self.app.poll_location(&self.clock, &mut self.location);
        if let Some(uptime) = self.app.tick(&self.clock) {
            self.heartbeats += 1;
            self.last_heartbeat_seconds = Some(uptime);
        }
        self.app
            .present(&mut PortraitDisplay::new(&mut self.display))
            .expect("application must not draw while display is busy");
    }

    /// Visits deadlines in order, including advances across many heartbeats.
    /// Large time advances give the same behavior as many small advances.
    pub fn advance(&mut self, duration_ms: u64) {
        let end = self.clock.now_ms.saturating_add(duration_ms);
        while self.clock.now_ms < end {
            let mut next = end.min(self.app.next_deadline_ms());
            if let Some(deadline) = self.discovery.next_deadline_ms() {
                next = next.min(deadline);
            }
            if let Some(deadline) = self.location.next_deadline_ms(self.clock.now_ms()) {
                next = next.min(deadline);
            }
            if self.display.is_busy() {
                next = next.min(self.display.ready_at_ms);
            }
            self.clock.now_ms = next;
            self.poll();
        }
    }

    /// Accepts input aligned with native panel axes from a calibrated touch driver.
    /// Uses the shared rotation path from hardware to application coordinates.
    pub fn touch_native(&mut self, event: TouchEvent) {
        self.app.touch(ROTATION.touch_to_portrait(event));
        self.poll();
    }

    /// Converts browser portrait coordinates into native touch input.
    /// Sends that input through the shared input adapter.
    pub fn touch_portrait(&mut self, event: TouchEvent) {
        let native = match event {
            TouchEvent::Down(point) => ROTATION.to_native(point).map(TouchEvent::Down),
            TouchEvent::Up(point) => ROTATION.to_native(point).map(TouchEvent::Up),
            TouchEvent::Cancel => None,
        };
        self.touch_native(native.unwrap_or(TouchEvent::Cancel));
    }

    /// Exports the completed native frame with a clockwise rotation.
    /// The application does not draw into this export buffer.
    pub fn portrait_png(&self) -> Result<Vec<u8>, png::EncodingError> {
        let mut portrait = vec![0; (SCREEN_SIZE.width * SCREEN_SIZE.height) as usize];
        for (index, pixel) in portrait.iter_mut().enumerate() {
            let point = Point::new(
                (index % SCREEN_SIZE.width as usize) as i32,
                (index / SCREEN_SIZE.width as usize) as i32,
            );
            let native = ROTATION
                .to_native(point)
                .expect("pixel inside portrait bounds");
            *pixel = self.display.pixels()
                [native.y as usize * NATIVE_SCREEN_SIZE.width as usize + native.x as usize];
        }
        encode_png(SCREEN_SIZE, &portrait)
    }

    pub fn display(&self) -> &SimDisplay {
        &self.display
    }

    pub fn debugger(&self) -> &Debugger {
        self.app.debugger()
    }

    pub fn set_scan_scenario(&mut self, scenario: ScanScenario) {
        self.discovery.scenario = scenario;
    }

    pub fn location(&self) -> &LocationScreen {
        self.app.location()
    }

    pub fn clock(&self) -> &impl Clock {
        &self.clock
    }

    /// Set or invalidate simulated calendar time without a monotonic correction.
    pub fn set_utc(&mut self, datetime: Option<UtcDateTime>) {
        self.clock.utc = datetime.map(|datetime| UtcReference {
            datetime,
            observed_at_ms: self.clock.now_ms(),
        });
    }

    pub fn set_location_scenario(&mut self, scenario: LocationScenario) {
        self.location.set_scenario(scenario, self.clock.now_ms());
        self.poll();
    }

    pub fn snapshot(&self) -> Snapshot {
        use paper_mesh_app::{
            BACK_TARGET, TAP_TARGET,
            debugger::{
                BLUETOOTH_TARGET, DEBUGGER_TARGET, NEXT_TARGET, PREVIOUS_TARGET, SCAN_TARGET,
                ScanState, WIFI_TARGET,
            },
        };
        let debugger = self.app.debugger();
        let location = self.app.location();
        let view = location.view();
        let targets: &[(&str, embedded_graphics::primitives::Rectangle)] = if location.is_open() {
            &[("Back", BACK_TARGET)]
        } else if debugger.is_open() {
            &[
                ("Back", BACK_TARGET),
                ("Wi-Fi", WIFI_TARGET),
                ("Bluetooth LE", BLUETOOTH_TARGET),
                ("Scan again", SCAN_TARGET),
                ("Previous", PREVIOUS_TARGET),
                ("Next", NEXT_TARGET),
            ]
        } else {
            &[
                ("Tap counter", TAP_TARGET),
                ("Radio debugger", DEBUGGER_TARGET),
                ("GNSS", paper_mesh_app::location::LOCATION_TARGET),
            ]
        };
        Snapshot {
            now_ms: self.clock.now_ms(),
            utc_datetime: self.clock.now_utc().map(|utc| {
                format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                    utc.year(),
                    utc.month() as u8,
                    utc.day(),
                    utc.hour(),
                    utc.minute(),
                    utc.second(),
                )
            }),
            taps: self.app.taps(),
            heartbeats: self.heartbeats,
            last_heartbeat_seconds: self.last_heartbeat_seconds,
            next_heartbeat_ms: self.app.next_heartbeat_ms(),
            display_busy: self.display.is_busy(),
            completed_refreshes: self.display.completed_refreshes,
            debugger_open: debugger.is_open(),
            scan_kind: match debugger.kind() {
                RadioKind::Wifi => "wifi",
                RadioKind::BluetoothLe => "bluetooth_le",
            },
            scan_state: match debugger.state() {
                ScanState::Idle => "idle",
                ScanState::Requested => "requested",
                ScanState::Active => "active",
                ScanState::Complete => "complete",
                ScanState::Error(_) => "error",
            },
            scan_source: debugger.source().map(|source| match source {
                ScanSource::Simulated => "simulated",
                ScanSource::Device => "device",
            }),
            scan_results: debugger.results().networks().len(),
            scan_observed_at_ms: debugger.observed_at_ms(),
            scan_scenario: self.discovery.scenario,
            location_open: location.is_open(),
            location_status: view.status.label(),
            latitude_microdegrees: view.coordinates.map(|value| value.latitude_microdegrees()),
            longitude_microdegrees: view.coordinates.map(|value| value.longitude_microdegrees()),
            elevation_mm: view.elevation_mm,
            speed_mm_s: view.speed_mm_s,
            course_millidegrees: view.course_millidegrees,
            utc_time: view
                .utc
                .map(|time| [time.hour(), time.minute(), time.second()]),
            satellites_used: view.satellites_used,
            location_age_seconds: view.age_seconds,
            receiver_status: view.receiver_status.label(),
            receiver_age_seconds: view.response_age_seconds,
            receiver_messages: view.valid_messages,
            satellites_in_view: view.satellites.map(|value| value.satellites_in_view),
            satellites_with_signal: view.satellites.map(|value| value.satellites_with_signal),
            satellite_age_seconds: view.satellite_age_seconds,
            satellite_signals: view
                .satellites
                .into_iter()
                .flat_map(|value| value.strongest.into_iter().flatten())
                .map(|value| SatelliteSnapshot {
                    system: satellite_label(value.system),
                    id: value.id,
                    cn0_db_hz: value.cn0_db_hz,
                })
                .collect(),
            location_scenario: self.location.scenario,
            controls: targets
                .iter()
                .map(|(label, target)| Control {
                    label,
                    x: target.center().x,
                    y: target.center().y,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use paper_mesh_app::BACK_TARGET;
    use paper_mesh_app::debugger::{
        BLUETOOTH_TARGET, DEBUGGER_TARGET, NEXT_TARGET, PREVIOUS_TARGET, SCAN_TARGET,
        SCAN_TIMEOUT_MS, ScanState,
    };
    use paper_mesh_platform::discovery::ScanError;

    fn press(sim: &mut Simulation, target: embedded_graphics::primitives::Rectangle) {
        sim.touch_portrait(TouchEvent::Down(target.center()));
        sim.touch_portrait(TouchEvent::Up(target.center()));
    }

    #[test]
    fn debugger_scans_both_radios_and_pages_without_changing_the_counter() {
        let mut sim = Simulation::default();
        sim.advance(REFRESH_MS);
        let home = sim.display.pixels().to_vec();
        press(&mut sim, DEBUGGER_TARGET);
        assert_eq!(sim.debugger().state(), ScanState::Active);
        assert_eq!(sim.debugger().source(), Some(ScanSource::Simulated));
        sim.advance(discovery::SCAN_MS - 1);
        assert!(sim.debugger().results().networks().is_empty());
        sim.advance(1);
        assert_eq!(sim.debugger().state(), ScanState::Complete);
        assert_eq!(
            sim.debugger().observed_at_ms(),
            Some(REFRESH_MS + discovery::SCAN_MS)
        );
        assert_eq!(sim.debugger().results().networks()[0].rssi_dbm, Some(-38));
        assert_eq!(
            sim.debugger().results().networks().last().unwrap().rssi_dbm,
            None
        );
        sim.advance(REFRESH_MS);
        assert_ne!(sim.display.pixels(), home);
        press(&mut sim, NEXT_TARGET);
        assert_eq!(sim.debugger().page(), 1);
        press(&mut sim, NEXT_TARGET);
        assert_eq!(sim.debugger().page(), 1);
        press(&mut sim, PREVIOUS_TARGET);
        assert_eq!(sim.debugger().page(), 0);
        press(&mut sim, BLUETOOTH_TARGET);
        assert!(sim.debugger().results().networks().is_empty());
        sim.advance(discovery::SCAN_MS + REFRESH_MS);
        assert_eq!(sim.debugger().kind(), RadioKind::BluetoothLe);
        assert_eq!(sim.debugger().results().networks()[0].name, "Sensor");
        assert_eq!(sim.snapshot().taps, 0);
        press(&mut sim, BACK_TARGET);
        assert!(!sim.debugger().is_open());
        tap(&mut sim);
        assert_eq!(sim.snapshot().taps, 1);
    }

    #[test]
    fn scan_scenarios_clear_old_results_and_allow_recovery() {
        let mut sim = Simulation::default();
        press(&mut sim, DEBUGGER_TARGET);
        sim.advance(discovery::SCAN_MS);
        for (scenario, expected, duration) in [
            (ScanScenario::Empty, ScanState::Complete, discovery::SCAN_MS),
            (
                ScanScenario::Failed,
                ScanState::Error(ScanError::Failed),
                discovery::SCAN_MS,
            ),
            (
                ScanScenario::Unavailable,
                ScanState::Error(ScanError::Unsupported),
                1,
            ),
            (
                ScanScenario::Timeout,
                ScanState::Error(ScanError::Timeout),
                SCAN_TIMEOUT_MS,
            ),
        ] {
            sim.set_scan_scenario(scenario);
            press(&mut sim, SCAN_TARGET);
            assert!(sim.debugger().results().networks().is_empty());
            sim.advance(duration);
            assert_eq!(sim.debugger().state(), expected);
        }
        sim.set_scan_scenario(ScanScenario::Nearby);
        press(&mut sim, SCAN_TARGET);
        sim.advance(discovery::SCAN_MS);
        assert_eq!(sim.debugger().state(), ScanState::Complete);
        assert_eq!(sim.debugger().results().networks().len(), 7);
    }

    #[test]
    fn scan_deadlines_and_frames_match_across_time_step_sizes() {
        let mut fast = Simulation::default();
        let mut slow = Simulation::default();
        press(&mut fast, DEBUGGER_TARGET);
        press(&mut slow, DEBUGGER_TARGET);
        fast.advance(30_500);
        for _ in 0..305 {
            slow.advance(100);
        }
        assert_eq!(fast.debugger().results(), slow.debugger().results());
        assert_eq!(
            fast.debugger().observed_at_ms(),
            slow.debugger().observed_at_ms()
        );
        assert_eq!(fast.display.pixels(), slow.display.pixels());
        assert_eq!(
            fast.snapshot().completed_refreshes,
            slow.snapshot().completed_refreshes
        );
    }

    #[test]
    fn a_tab_change_or_back_cancels_the_old_scan() {
        let mut sim = Simulation::default();
        press(&mut sim, DEBUGGER_TARGET);
        sim.advance(500);
        press(&mut sim, BLUETOOTH_TARGET);
        sim.advance(1_000);
        assert_eq!(sim.debugger().state(), ScanState::Active);
        sim.advance(500);
        assert_eq!(sim.debugger().state(), ScanState::Complete);
        assert_eq!(sim.debugger().results().networks()[0].name, "Sensor");
        press(&mut sim, SCAN_TARGET);
        press(&mut sim, BACK_TARGET);
        assert_eq!(sim.discovery.next_deadline_ms(), None);
        press(&mut sim, DEBUGGER_TARGET);
        sim.advance(discovery::SCAN_MS);
        assert_eq!(sim.debugger().state(), ScanState::Complete);
    }

    #[test]
    fn partial_tap_updates_match_simulator_and_preserve_pixels_outside_counter() {
        use embedded_graphics::primitives::Rectangle;
        use paper_mesh_ed047tc1::{
            FRAME_BYTES, FrameHistory, Framebuffer, ROW_BYTES, Update, WIDTH,
        };

        let mut sim = Simulation::default();
        sim.advance(REFRESH_MS);
        let mut staging = vec![0; FRAME_BYTES];
        let mut previous = vec![0; FRAME_BYTES];
        let mut frame = Framebuffer::new(staging.as_mut_slice().try_into().unwrap());
        let mut history = FrameHistory::new(previous.as_mut_slice().try_into().unwrap());
        sim.app
            .render(&mut PortraitDisplay::new(&mut frame))
            .unwrap();
        history.commit(&frame);
        let mut visible = sim.display.pixels().to_vec();
        let counter = Rectangle::new(Point::new(48, 490), Size::new(444, 30));
        // Include 9 -> 10. The centered label moves when its width changes.
        for count in 1..=12 {
            tap(&mut sim);
            sim.advance(REFRESH_MS);
            sim.app
                .render(&mut PortraitDisplay::new(&mut frame))
                .unwrap();
            let Update::Partial(damage) = history.plan(&frame) else {
                panic!("tap {count} did not select partial refresh");
            };
            assert!(damage.pixels > 0);
            let mut driven = 0;
            let mut row = [0xff; ROW_BYTES];
            for y in 0..540 {
                assert!(history.encode_partial_row(&frame, y, &mut row));
                for x in 0..WIDTH {
                    let drive = (row[x / 4] >> (2 * (x % 4))) & 3;
                    if drive == 0 {
                        continue;
                    }
                    let logical = ROTATION
                        .to_portrait(Point::new(x as i32, y as i32))
                        .unwrap();
                    assert!(
                        counter.contains(logical),
                        "tap {count}: changed {logical:?}"
                    );
                    visible[y * WIDTH + x] = match drive {
                        1 => 0,
                        2 => 255,
                        _ => panic!("invalid drive command"),
                    };
                    driven += 1;
                }
            }
            assert_eq!(driven, damage.pixels);
            assert_eq!(visible, sim.display.pixels(), "tap {count}");
            history.commit(&frame);
            assert_eq!(history.plan(&frame), Update::Unchanged);
        }
    }

    #[test]
    fn hardware_framebuffer_matches_every_simulator_pixel() {
        use paper_mesh_ed047tc1::{FRAME_BYTES, Framebuffer};

        let mut sim = Simulation::default();
        sim.advance(REFRESH_MS);
        let mut storage = vec![0; FRAME_BYTES];
        let mut frame = Framebuffer::new(storage.as_mut_slice().try_into().unwrap());
        for uptime in [0, 30, 60] {
            if uptime > 0 {
                sim.advance(30_000);
            }
            sim.app
                .render(&mut PortraitDisplay::new(&mut frame))
                .unwrap();
            for (index, pixel) in sim.display.pixels().iter().enumerate() {
                let shade = (frame.bytes()[index / 2] >> (4 * (index & 1))) & 15;
                assert_eq!(shade * 17, *pixel, "native pixel {index}, uptime {uptime}");
            }
            println!("uptime={uptime}s frame_fnv1a={:08x}", frame.checksum());
        }
    }

    fn tap(sim: &mut Simulation) {
        // Native (480, 269) maps to portrait (270, 480), in the button.
        let point = Point::new(480, 269);
        sim.touch_native(TouchEvent::Down(point));
        sim.touch_native(TouchEvent::Up(point));
    }

    #[test]
    fn a_frame_is_visible_only_after_refresh_and_idle_does_not_refresh() {
        let mut sim = Simulation::default();
        assert!(sim.display.pixels().iter().all(|pixel| *pixel == 255));
        sim.advance(REFRESH_MS - 1);
        assert_eq!(sim.snapshot().completed_refreshes, 0);
        sim.advance(1);
        assert!(sim.display.pixels().contains(&0));
        assert_eq!(sim.snapshot().completed_refreshes, 1);
        sim.advance(10_000);
        assert_eq!(sim.snapshot().completed_refreshes, 1);
    }

    #[test]
    fn input_during_refresh_is_coalesced_into_the_next_frame() {
        let mut sim = Simulation::default();
        tap(&mut sim);
        tap(&mut sim);
        tap(&mut sim);
        assert_eq!(sim.snapshot().taps, 3);
        sim.advance(REFRESH_MS);
        assert_eq!(sim.snapshot().completed_refreshes, 1);
        assert!(sim.snapshot().display_busy);
        sim.advance(REFRESH_MS);
        assert_eq!(sim.snapshot().completed_refreshes, 2);
        assert!(!sim.snapshot().display_busy);
        let mut expected = SimDisplay::default();
        sim.app
            .render(&mut PortraitDisplay::new(&mut expected))
            .unwrap();
        assert_eq!(sim.display.pixels(), expected.drawing);
    }

    #[test]
    fn rapid_taps_survive_multiple_pending_refreshes() {
        let mut sim = Simulation::default();
        let point = Point::new(270, 480);
        for count in 1..=40 {
            sim.touch_portrait(TouchEvent::Down(point));
            sim.advance(30);
            sim.touch_portrait(TouchEvent::Up(point));
            assert_eq!(sim.snapshot().taps, count);
            sim.advance(30);
        }
        sim.advance(2 * REFRESH_MS);
        assert!(!sim.snapshot().display_busy);
        assert!(sim.snapshot().completed_refreshes < 40);
        let mut expected = SimDisplay::default();
        sim.app
            .render(&mut PortraitDisplay::new(&mut expected))
            .unwrap();
        assert_eq!(sim.display.pixels(), expected.drawing);
    }

    #[test]
    fn fast_forward_matches_small_steps_including_frames() {
        let mut fast = Simulation::default();
        let mut slow = Simulation::default();
        fast.advance(90_500);
        for _ in 0..905 {
            slow.advance(100);
        }
        assert_eq!(fast.snapshot().heartbeats, 3);
        assert_eq!(fast.snapshot().last_heartbeat_seconds, Some(90));
        assert_eq!(fast.snapshot().completed_refreshes, 4);
        assert_eq!(fast.display.pixels(), slow.display.pixels());
        assert_eq!(
            fast.snapshot().completed_refreshes,
            slow.snapshot().completed_refreshes
        );
    }

    #[test]
    fn png_preserves_dimensions_and_grayscale_pixels() {
        let mut sim = Simulation::default();
        sim.advance(REFRESH_MS);
        let encoded = sim.display.native_png().unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(encoded))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!((info.width, info.height), (960, 540));
        assert_eq!(info.color_type, png::ColorType::Grayscale);
        assert_eq!(&pixels[..info.buffer_size()], sim.display.pixels());
        assert!(pixels.iter().all(|pixel| pixel % 17 == 0));
    }

    #[test]
    fn portrait_export_matches_direct_render_after_native_refresh() {
        assert_eq!(SCREEN_SIZE, paper_mesh_app::SCREEN_SIZE);
        // This independent portrait reference target uses no rotation code.
        struct Reference(Vec<u8>);
        impl OriginDimensions for Reference {
            fn size(&self) -> Size {
                SCREEN_SIZE
            }
        }
        impl DrawTarget for Reference {
            type Color = Gray4;
            type Error = core::convert::Infallible;

            fn draw_iter<I: IntoIterator<Item = Pixel<Gray4>>>(
                &mut self,
                pixels: I,
            ) -> Result<(), Self::Error> {
                for Pixel(point, color) in pixels {
                    if self.bounding_box().contains(point) {
                        self.0[point.y as usize * 540 + point.x as usize] = color.luma() * 17;
                    }
                }
                Ok(())
            }
        }

        let mut sim = Simulation::default();
        sim.advance(REFRESH_MS);
        let before = sim.portrait_png().unwrap();
        tap(&mut sim);
        sim.advance(REFRESH_MS - 1);
        assert_eq!(sim.portrait_png().unwrap(), before);
        sim.advance(1);

        let mut expected = Reference(vec![0; 540 * 960]);
        sim.app.render(&mut expected).unwrap();
        let encoded = sim.portrait_png().unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(encoded))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!((info.width, info.height), (540, 960));
        assert_eq!(&pixels[..info.buffer_size()], expected.0);
        assert_ne!(sim.display.pixels(), expected.0);
    }

    #[test]
    fn native_and_browser_touches_share_rotation_and_cancellation() {
        let mut sim = Simulation::default();
        tap(&mut sim);
        assert_eq!(sim.snapshot().taps, 1);
        sim.touch_portrait(TouchEvent::Down(Point::new(270, 480)));
        sim.touch_portrait(TouchEvent::Up(Point::new(270, 480)));
        assert_eq!(sim.snapshot().taps, 2);

        sim.touch_native(TouchEvent::Down(Point::new(480, 269)));
        sim.touch_native(TouchEvent::Up(Point::new(960, 269)));
        sim.touch_native(TouchEvent::Up(Point::new(480, 269)));
        sim.touch_portrait(TouchEvent::Down(Point::new(270, 480)));
        sim.touch_portrait(TouchEvent::Cancel);
        sim.touch_portrait(TouchEvent::Up(Point::new(270, 480)));
        sim.touch_native(TouchEvent::Down(Point::new(0, 539)));
        sim.touch_native(TouchEvent::Up(Point::new(480, 269)));
        assert_eq!(sim.snapshot().taps, 2);
    }

    #[test]
    fn failed_refresh_is_retried() {
        struct FailingDisplay {
            inner: SimDisplay,
            fail: bool,
        }
        impl OriginDimensions for FailingDisplay {
            fn size(&self) -> Size {
                NATIVE_SCREEN_SIZE
            }
        }
        impl DrawTarget for FailingDisplay {
            type Color = Gray4;
            type Error = &'static str;
            fn draw_iter<I: IntoIterator<Item = Pixel<Gray4>>>(
                &mut self,
                pixels: I,
            ) -> Result<(), Self::Error> {
                self.inner.draw_iter(pixels).map_err(|_| "busy")
            }
        }
        impl Display for FailingDisplay {
            fn is_busy(&self) -> bool {
                self.inner.is_busy()
            }
            fn refresh(&mut self) -> Result<(), Self::Error> {
                if self.fail {
                    self.fail = false;
                    Err("injected refresh failure")
                } else {
                    self.inner.refresh().map_err(|_| "busy")
                }
            }
        }
        let mut app = App::new(&VirtualClock::default());
        let mut display = FailingDisplay {
            inner: SimDisplay::default(),
            fail: true,
        };
        assert!(
            app.present(&mut PortraitDisplay::new(&mut display))
                .is_err()
        );
        assert_eq!(
            app.present(&mut PortraitDisplay::new(&mut display)),
            Ok(true)
        );
        display.inner.advance_to(REFRESH_MS);
        assert_eq!(
            app.present(&mut PortraitDisplay::new(&mut display)),
            Ok(false)
        );
        assert!(display.inner.pixels().contains(&0));
    }
}
