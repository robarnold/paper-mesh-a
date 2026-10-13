//! Native adapters and deterministic simulation. HTTP and wall-clock pacing
//! live in the binary; scenarios can drive this library without either.

use embedded_graphics::{pixelcolor::Gray4, prelude::*};
use paper_mesh_app::{
    App,
    platform::{
        Clock, Display, NATIVE_SCREEN_SIZE, SCREEN_SIZE, TouchEvent,
        rotation::{PortraitDisplay, PortraitRotation},
    },
};
use serde::Serialize;

// An artificial delay to exercise asynchronous refresh, not a panel measurement.
pub const REFRESH_MS: u64 = 250;
const ROTATION: PortraitRotation = PortraitRotation::new(NATIVE_SCREEN_SIZE);

#[derive(Default)]
pub struct VirtualClock(u64);

impl Clock for VirtualClock {
    fn now_ms(&self) -> u64 {
        self.0
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
    pub taps: u32,
    pub heartbeats: u64,
    pub last_heartbeat_seconds: Option<u64>,
    pub next_heartbeat_ms: u64,
    pub display_busy: bool,
    pub completed_refreshes: u64,
}

pub struct Simulation {
    app: App,
    clock: VirtualClock,
    display: SimDisplay,
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
        if let Some(uptime) = self.app.tick(&self.clock) {
            self.heartbeats += 1;
            self.last_heartbeat_seconds = Some(uptime);
        }
        self.app
            .present(&mut PortraitDisplay::new(&mut self.display))
            .expect("application must not draw while display is busy");
    }

    /// Visit deadlines in order, even when fast-forwarding over many heartbeats.
    /// This gives scenarios the same behavior as many small time advances.
    pub fn advance(&mut self, duration_ms: u64) {
        let end = self.clock.0.saturating_add(duration_ms);
        while self.clock.0 < end {
            let mut next = end.min(self.app.next_deadline_ms());
            if self.display.is_busy() {
                next = next.min(self.display.ready_at_ms);
            }
            self.clock.0 = next;
            self.poll();
        }
    }

    /// Input aligned with the native panel axes, as supplied by a calibrated
    /// touch driver. This is the shared hardware-to-application rotation path.
    pub fn touch_native(&mut self, event: TouchEvent) {
        self.app.touch(ROTATION.touch_to_portrait(event));
        self.poll();
    }

    /// The browser sees the device held in portrait. Generate native touch
    /// input from that view, then feed it through the same input adapter.
    pub fn touch_portrait(&mut self, event: TouchEvent) {
        let native = match event {
            TouchEvent::Down(point) => ROTATION.to_native(point).map(TouchEvent::Down),
            TouchEvent::Up(point) => ROTATION.to_native(point).map(TouchEvent::Up),
            TouchEvent::Cancel => None,
        };
        self.touch_native(native.unwrap_or(TouchEvent::Cancel));
    }

    /// Present the completed native frame as if the panel were held clockwise.
    /// This export-only buffer is not an application drawing surface.
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

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            now_ms: self.clock.now_ms(),
            taps: self.app.taps(),
            heartbeats: self.heartbeats,
            last_heartbeat_seconds: self.last_heartbeat_seconds,
            next_heartbeat_ms: self.app.next_deadline_ms(),
            display_busy: self.display.is_busy(),
            completed_refreshes: self.display.completed_refreshes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tap(sim: &mut Simulation) {
        // Native (480, 269) corresponds to portrait (270, 480), inside the button.
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
        // Independent portrait reference target: no rotation code is used here.
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
