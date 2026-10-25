// SPDX-License-Identifier: GPL-3.0-only
// The bus sequence derives from lilygo-t5s3paperpro-rs at c9cf0be45a33c4b2574aa3dde6dc128bb7ed7233.
// Copyright Andrew Whaley and Frido Koch. See ../../crates/ed047tc1/README.md.
//! H752-01 display adapter with synchronous full and partial refresh.

use alloc::boxed::Box;
use embedded_graphics::{pixelcolor::Gray4, prelude::*};
use esp_hal::{
    delay::Delay,
    dma::DmaTxBuf,
    dma_tx_buffer,
    gpio::Level,
    lcd_cam::lcd::i8080::Command,
    peripherals,
    psram::{Psram, PsramConfig, PsramMode},
    rmt::{PulseCode, TxTransaction},
    time::{Duration, Instant},
};
use log::{error, info};
use paper_mesh_ed047tc1::{
    FRAME_BYTES, FrameHistory, Framebuffer, HEIGHT, PARTIAL_PHASE_TICKS, PHASE_TICKS, ROW_BYTES,
    STORAGE_BYTES, Update, WIDTH,
};
use paper_mesh_platform::Display;
mod scanout;
use scanout::{Resources, Scanout};

mod power_sequence;
use power_sequence::{MODE, OE};

pub const NATIVE_SCREEN_SIZE: Size = Size::new(WIDTH as u32, HEIGHT as u32);

#[derive(Debug)]
pub enum Error {
    I2c(esp_hal::i2c::master::Error),
    Configuration,
    Dma,
    Rmt,
    Timeout(&'static str),
    Temperature(i8),
    Psram,
    PartialTransition,
    Busy,
    PowerVerification(&'static str),
    Faulted,
    SupplyFault(u8, u8),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::I2c(error) => write!(f, "I2C: {error:?}"),
            Self::Timeout(stage) => write!(f, "timeout: {stage}"),
            Self::Temperature(value) => write!(f, "temperature outside 0..50 C: {value}"),
            Self::PowerVerification(stage) => write!(f, "power verification: {stage}"),
            Self::SupplyFault(a, b) => write!(f, "PMIC fault: INT1=0x{a:02x}; INT2=0x{b:02x}"),
            other => write!(f, "{other:?}"),
        }
    }
}

pub struct Pins {
    pub data0: peripherals::GPIO5<'static>,
    pub data1: peripherals::GPIO6<'static>,
    pub data2: peripherals::GPIO7<'static>,
    pub data3: peripherals::GPIO15<'static>,
    pub data4: peripherals::GPIO16<'static>,
    pub data5: peripherals::GPIO17<'static>,
    pub data6: peripherals::GPIO18<'static>,
    pub data7: peripherals::GPIO8<'static>,
    pub latch: peripherals::GPIO42<'static>,
    pub start_horizontal: peripherals::GPIO41<'static>,
    pub clock_horizontal: peripherals::GPIO4<'static>,
    pub clock_vertical: peripherals::GPIO48<'static>,
    pub start_vertical: peripherals::GPIO45<'static>,
}

/// Clear retained controls before heap, PSRAM, or scan driver setup.
pub fn establish_controls_off(bus: &crate::bus::Bus) -> Result<(), Error> {
    power_sequence::State::new()
        .establish_off(&mut PowerAccess::new(bus))
        .map_err(power_error)
}

pub fn establish_safe_state(bus: &crate::bus::Bus, pins: &mut Pins) -> Result<(), Error> {
    let mut power = power_sequence::State::new();
    let blank = power.blank_retained_outputs(&mut PowerAccess::new(bus));
    pins.idle();
    let off = power.establish_off(&mut PowerAccess::new(bus));
    blank.and(off).map_err(power_error)
}

struct PowerAccess<'a> {
    bus: &'a crate::bus::Bus,
}

impl<'a> PowerAccess<'a> {
    fn new(bus: &'a crate::bus::Bus) -> Self {
        Self { bus }
    }
}

impl power_sequence::Access for PowerAccess<'_> {
    type Error = Error;

    fn read(&mut self, device: u8, register: u8) -> Result<u8, Error> {
        let mut value = [0];
        lock_bus(self.bus)?
            .write_read(device, &[register], &mut value)
            .map_err(Error::I2c)?;
        Ok(value[0])
    }

    fn write(&mut self, device: u8, register: u8, value: u8) -> Result<(), Error> {
        lock_bus(self.bus)?
            .write(device, &[register, value])
            .map_err(Error::I2c)
    }

    fn wait_ms(&mut self, milliseconds: u32) {
        Delay::new().delay_millis(milliseconds);
    }

    fn now_ms(&self) -> u64 {
        Instant::now().duration_since_epoch().as_millis()
    }

    fn mark(&mut self, stage: power_sequence::Stage) {
        use crate::display_recovery::Stage;
        let stage = match stage {
            power_sequence::Stage::LogicReady => Stage::LogicReady,
            power_sequence::Stage::RailsRequested => Stage::RailsRequested,
            power_sequence::Stage::RailsReady => Stage::RailsReady,
            power_sequence::Stage::HighVoltageOff | power_sequence::Stage::WakeLow => return,
        };
        crate::display_recovery::mark(stage);
    }
}

fn lock_bus(bus: &crate::bus::Bus) -> Result<crate::bus::BusGuard<'_>, Error> {
    let deadline = Instant::now() + Duration::from_millis(100);
    bus.lock_blocking(|| {
        if Instant::now() >= deadline {
            return false;
        }
        Delay::new().delay_micros(10);
        true
    })
    .ok_or(Error::Timeout("I2C bus ownership"))
}

fn power_error(error: power_sequence::Error<Error>) -> Error {
    match error {
        power_sequence::Error::Transport(error) => error,
        power_sequence::Error::Verification(stage) => Error::PowerVerification(stage),
        power_sequence::Error::Timeout(stage) => Error::Timeout(stage),
        power_sequence::Error::Temperature(value) => Error::Temperature(value),
        power_sequence::Error::Faulted => Error::Faulted,
        power_sequence::Error::SupplyFault(a, b) => Error::SupplyFault(a, b),
    }
}

pub struct Panel {
    frame: Framebuffer<'static>,
    history: FrameHistory<'static>,
    _psram: Psram,
    bus: &'static crate::bus::Bus,
    scanout: Option<Resources>,
    dma: Option<DmaTxBuf>,
    power: Box<power_sequence::State>,
}

impl Panel {
    pub fn frame_checksum(&self) -> u32 {
        self.frame.checksum()
    }

    pub fn resume_from_suspend(&mut self, before: u32) {
        let after = self.frame.checksum();
        info!(
            "display: suspend frame {:08x}->{:08x}; retained={}; next refresh full",
            before,
            after,
            before == after
        );
        // Establish the panel baseline again after the clocks stop.
        self.history.invalidate();
    }

    pub fn new(
        pins: Pins,
        psram: peripherals::PSRAM<'static>,
        bus: &'static crate::bus::Bus,
        lcd: peripherals::LCD_CAM<'static>,
        dma: peripherals::DMA_CH0<'static>,
        rmt: peripherals::RMT<'static>,
    ) -> Result<Self, Error> {
        let psram = Psram::new(
            psram,
            PsramConfig {
                mode: PsramMode::OctalSpi,
                ..Default::default()
            },
        );
        let (address, size) = psram.raw_parts();

        if address.is_null() || size < STORAGE_BYTES {
            return Err(Error::Psram);
        }
        // SAFETY: This adapter owns PSRAM and creates its only memory reference.
        // The checked region contains STORAGE_BYTES bytes with alignment one.
        // No allocator or DMA accesses this region. The adapter retains Psram.
        let storage = unsafe {
            address.write_bytes(0xff, STORAGE_BYTES);
            core::slice::from_raw_parts_mut(address, STORAGE_BYTES)
        };
        let (staging, previous) = storage.split_at_mut(FRAME_BYTES);
        let frame = Framebuffer::new(staging.try_into().unwrap());
        let history = FrameHistory::new(previous.try_into().unwrap());
        if frame.bytes().iter().any(|byte| *byte != 0xff) {
            return Err(Error::Psram);
        }
        info!(
            "display: PSRAM={} bytes; framebuffers={} bytes",
            size, STORAGE_BYTES
        );

        let mut scanout = Resources::new(pins, lcd, dma, rmt);
        // Open scan drivers only while panel logic power is present.
        scanout.idle();
        // The eight trailing bytes flush the source-driver pipeline.
        let dma = dma_tx_buffer!(ROW_BYTES + 8).map_err(|_| Error::Dma)?;
        let panel = Self {
            frame,
            history,
            _psram: psram,
            bus,
            scanout: Some(scanout),
            dma: Some(dma),
            power: Box::new(power_sequence::State::new()),
        };
        panel
            .power
            .verify_off(&mut PowerAccess::new(bus))
            .map_err(power_error)?;
        info!("display: ED047TC1 bus initialized; native=960x540; logical=540x960");
        Ok(panel)
    }

    fn set_control(&mut self, mask: u8, value: u8) -> Result<(), Error> {
        self.power
            .set_control(&mut PowerAccess::new(self.bus), mask, value)
            .map_err(power_error)
    }

    fn power_on(&mut self) -> Result<(), Error> {
        let magnitude_mv = 1600;
        let reading = self
            .power
            .power_on(&mut PowerAccess::new(self.bus), magnitude_mv)
            .map_err(power_error)?;
        info!(
            "display: TPS65185 revision=0x{:02x}; temperature={} C; rails=0xfa; VCOM1=0x{:02x}; VCOM2=0x{:02x}",
            reading.revision, reading.temperature, reading.vcom1, reading.vcom2
        );
        Ok(())
    }

    fn power_off(&mut self) -> Result<(), Error> {
        let mut access = PowerAccess::new(self.bus);
        self.power
            .stop_high_voltage(&mut access)
            .map_err(power_error)
    }

    fn load_neutral(&mut self, io: &mut Scanout<'_>) -> Result<(), Error> {
        let mut dma = self.dma.take().ok_or(Error::Dma)?;
        dma.as_mut_slice().fill(0);
        let Some(lcd) = io.lcd.take() else {
            self.dma = Some(dma);
            return Err(Error::Dma);
        };
        let tx = match lcd.send(Command::<u8>::One(0), 0, dma) {
            Ok(tx) => tx,
            Err((_, lcd, dma)) => {
                io.lcd = Some(lcd);
                self.dma = Some(dma);
                return Err(Error::Dma);
            }
        };
        let deadline = Instant::now() + Duration::from_millis(20);
        while !tx.is_done() {
            if Instant::now() >= deadline {
                let (lcd, dma) = tx.cancel();
                io.lcd = Some(lcd);
                self.dma = Some(dma);
                return Err(Error::Timeout("neutral LCD DMA"));
            }
        }
        let (result, lcd, dma) = tx.wait();
        io.lcd = Some(lcd);
        self.dma = Some(dma);
        result.map_err(|_| Error::Dma)?;
        io.latch.set_high();
        Delay::new().delay_micros(1);
        io.latch.set_low();
        Ok(())
    }

    fn pulse(&mut self, io: &mut Scanout<'_>, high: u16, low: u16) -> Result<(), Error> {
        let data = [
            PulseCode::new(Level::High, high, Level::Low, low),
            PulseCode::end_marker(),
        ];
        let tx = io
            .clock_vertical
            .reborrow()
            .transmit(&data)
            .map_err(|_| Error::Rmt)?;
        wait_pulse(tx)
    }

    fn frame_start(&mut self, io: &mut Scanout<'_>) -> Result<(), Error> {
        self.set_control(MODE, MODE)?;
        self.pulse(io, 10, 10)?;
        io.start_vertical.set_low();
        Delay::new().delay_micros(1);
        let data = [
            PulseCode::new(Level::High, 100, Level::Low, 100),
            PulseCode::end_marker(),
        ];
        let tx = io
            .clock_vertical
            .reborrow()
            .transmit(&data)
            .map_err(|_| Error::Rmt)?;
        io.start_vertical.set_high();
        wait_pulse(tx)?;
        self.pulse(io, 100, 0)?;
        self.set_control(OE, OE)?;
        self.pulse(io, 10, 10)?;
        Ok(())
    }

    fn frame_end(&mut self, io: &mut Scanout<'_>) -> Result<(), Error> {
        self.set_control(OE | MODE, 0)?;
        self.pulse(io, 10, 10)?;
        self.pulse(io, 10, 10)?;
        Ok(())
    }

    fn output_row(
        &mut self,
        io: &mut Scanout<'_>,
        row: &[u8; ROW_BYTES],
        ticks: u16,
    ) -> Result<(), Error> {
        let mut dma = self.dma.take().ok_or(Error::Dma)?;
        dma.as_mut_slice()[..ROW_BYTES].copy_from_slice(row);
        dma.as_mut_slice()[ROW_BYTES..].fill(0);
        // The queued bytes latch on the next row.
        io.latch.set_high();
        Delay::new().delay_micros(1);
        io.latch.set_low();
        Delay::new().delay_micros(1);
        let data = [
            PulseCode::new(Level::High, ticks, Level::Low, 50),
            PulseCode::end_marker(),
        ];
        let pulse = match io.clock_vertical.reborrow().transmit(&data) {
            Ok(pulse) => pulse,
            Err(_) => {
                self.dma = Some(dma);
                return Err(Error::Rmt);
            }
        };
        let Some(lcd) = io.lcd.take() else {
            self.dma = Some(dma);
            return Err(Error::Dma);
        };
        let tx = match lcd.send(Command::<u8>::One(0), 0, dma) {
            Ok(tx) => tx,
            Err((_, lcd, dma)) => {
                io.lcd = Some(lcd);
                self.dma = Some(dma);
                return Err(Error::Dma);
            }
        };
        let deadline = Instant::now() + Duration::from_millis(20);
        while !tx.is_done() {
            if Instant::now() >= deadline {
                let (lcd, dma) = tx.cancel();
                io.lcd = Some(lcd);
                self.dma = Some(dma);
                return Err(Error::Timeout("LCD DMA"));
            }
        }
        let (result, lcd, dma) = tx.wait();
        io.lcd = Some(lcd);
        self.dma = Some(dma);
        let pulse_result = wait_pulse(pulse);
        result.map_err(|_| Error::Dma).and(pulse_result)
    }

    fn scan_uniform(&mut self, io: &mut Scanout<'_>, drive: u8) -> Result<(), Error> {
        self.frame_start(io)?;
        let row = [drive; ROW_BYTES];
        for _ in 0..=HEIGHT {
            self.output_row(io, &row, 50)?;
        }
        self.frame_end(io)
    }

    fn update_full(&mut self, io: &mut Scanout<'_>) -> Result<(), Error> {
        // Restore a white baseline with four black scans, then four white scans.

        for _ in 0..4 {
            for _ in 0..4 {
                self.scan_uniform(io, 0x55)?;
            }
            for _ in 0..4 {
                self.scan_uniform(io, 0xaa)?;
            }
        }
        let mut row = [0; ROW_BYTES];
        for (phase, ticks) in PHASE_TICKS.into_iter().enumerate() {
            self.frame_start(io)?;
            for y in 0..HEIGHT {
                self.frame.encode_row(y, phase, &mut row);
                self.output_row(io, &row, ticks)?;
            }
            self.output_row(io, &[0; ROW_BYTES], ticks)?;
            self.frame_end(io)?;
        }
        Ok(())
    }

    fn update_partial(&mut self, io: &mut Scanout<'_>) -> Result<(), Error> {
        let mut row = [0; ROW_BYTES];
        for ticks in PARTIAL_PHASE_TICKS {
            self.frame_start(io)?;
            for y in 0..HEIGHT {
                if !self.history.encode_partial_row(&self.frame, y, &mut row) {
                    return Err(Error::PartialTransition);
                }
                // Traverse every gate row. Unchanged pixels receive neutral commands.
                self.output_row(io, &row, ticks)?;
            }
            self.output_row(io, &[0; ROW_BYTES], ticks)?;
            self.frame_end(io)?;
        }
        Ok(())
    }
}

fn wait_pulse(mut tx: TxTransaction<'_, '_>) -> Result<(), Error> {
    let deadline = Instant::now() + Duration::from_millis(20);
    while !tx.poll() {
        if Instant::now() >= deadline {
            return Err(Error::Timeout("RMT"));
        }
    }
    tx.wait().map(|_| ()).map_err(|_| Error::Rmt)
}

impl OriginDimensions for Panel {
    fn size(&self) -> Size {
        self.frame.size()
    }
}

impl DrawTarget for Panel {
    type Color = Gray4;
    type Error = Error;

    fn draw_iter<I: IntoIterator<Item = Pixel<Gray4>>>(&mut self, pixels: I) -> Result<(), Error> {
        self.frame.draw_iter(pixels).unwrap();
        Ok(())
    }

    fn clear(&mut self, color: Gray4) -> Result<(), Error> {
        self.frame.clear(color).unwrap();
        Ok(())
    }
}

impl Display for Panel {
    fn is_busy(&self) -> bool {
        false
    }

    fn refresh(&mut self) -> Result<(), Error> {
        if self.power.faulted() || crate::display_recovery::blocked() {
            return Err(Error::Faulted);
        }
        let start = Instant::now();
        let update = self.history.plan(&self.frame);
        let mode = match update {
            Update::Unchanged => {
                info!("display: unchanged frame; refresh skipped");
                return Ok(());
            }
            Update::Full => "full",
            Update::Partial(damage) => {
                info!(
                    "display: damage native=({}, {}) {}x{}; changed_pixels={}",
                    damage.area.top_left.x,
                    damage.area.top_left.y,
                    damage.area.size.width,
                    damage.area.size.height,
                    damage.pixels
                );
                "partial"
            }
        };
        info!(
            "display: refresh start; mode={}; frame_fnv1a={:08x}",
            mode,
            self.frame.checksum()
        );
        self.history.invalidate();

        let guard = crate::display_recovery::Guard::begin().map_err(|_| Error::Faulted)?;
        let mut resources = self.scanout.take().ok_or(Error::Busy)?;
        let mut shutdown_confirmed = false;
        let mut shutdown_attempted = false;

        let result = self.power_on().and_then(|()| {
            resources.open().and_then(|mut io| {
                info!("display: scanout drivers ready");
                let result = self.load_neutral(&mut io).and_then(|()| {
                    io.start_vertical.set_high();
                    crate::display_recovery::mark(crate::display_recovery::Stage::Scanning);
                    match update {
                        Update::Full => self.update_full(&mut io),
                        Update::Partial(_) => self.update_partial(&mut io),
                        Update::Unchanged => unreachable!(),
                    }
                });
                crate::display_recovery::mark(crate::display_recovery::Stage::ShuttingDown);
                io.start_vertical.set_low();
                io.latch.set_low();
                shutdown_attempted = true;
                let power_off = self.power_off();
                shutdown_confirmed = power_off.is_ok();
                if let Err(error) = &power_off {
                    error!("display: power off failed: {}", error);
                }
                result.and(power_off)
            })
        });
        if !shutdown_attempted {
            crate::display_recovery::mark(crate::display_recovery::Stage::ShuttingDown);
            shutdown_confirmed = self.power_off().is_ok();
        }
        // Dropping the temporary drivers releases both clock locks on success or error.
        resources.idle();
        let mut access = PowerAccess::new(self.bus);
        let finish_off = self.power.finish_off(&mut access).map_err(power_error);
        shutdown_confirmed &= finish_off.is_ok();
        let result = result.and(finish_off);
        let verification = self
            .power
            .verify_off(&mut PowerAccess::new(self.bus))
            .map_err(power_error);
        if verification.is_err() {
            shutdown_confirmed = false;
            self.power.latch_fault();
        }
        let result = result.and(verification);
        if shutdown_confirmed {
            crate::display_recovery::mark(crate::display_recovery::Stage::ControlsLow);
        }

        guard.finish(shutdown_confirmed, result.is_ok());
        self.scanout = Some(resources);
        result?;
        self.history.commit(&self.frame);
        info!(
            "display: refresh complete; mode={}; elapsed={} ms; power off",
            mode,
            start.elapsed().as_millis()
        );
        Ok(())
    }
}
