// SPDX-License-Identifier: GPL-3.0-only
// The bus sequence derives from lilygo-t5s3paperpro-rs at c9cf0be45a33c4b2574aa3dde6dc128bb7ed7233.
// Copyright Andrew Whaley and Frido Koch. See ../../crates/ed047tc1/README.md.
//! H752-01 display adapter with synchronous full and partial refresh.

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

pub const NATIVE_SCREEN_SIZE: Size = Size::new(WIDTH as u32, HEIGHT as u32);

const EXPANDER: u8 = 0x20;
const SUPPLY: u8 = 0x68;
const OE: u8 = 1;
const MODE: u8 = 2;
const PWRUP: u8 = 8;
const VCOM: u8 = 16;
const WAKE: u8 = 32;
const DISPLAY_OUTPUTS: u8 = OE | MODE | PWRUP | VCOM | WAKE;
const PG: u8 = 64;

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
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::I2c(error) => write!(f, "I2C: {error:?}"),
            Self::Timeout(stage) => write!(f, "timeout: {stage}"),
            Self::Temperature(value) => write!(f, "temperature outside 0..50 C: {value}"),
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

pub struct Panel {
    frame: Framebuffer<'static>,
    history: FrameHistory<'static>,
    _psram: Psram,
    bus: &'static crate::bus::Bus,
    scanout: Option<Resources>,
    dma: Option<DmaTxBuf>,
    control: u8,
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
            &mut *address.cast::<[u8; STORAGE_BYTES]>()
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
        // Validate configuration without retaining the LCD and RMT clock wake locks.
        drop(scanout.open()?);
        // The eight trailing bytes flush the source-driver pipeline.
        let dma = dma_tx_buffer!(ROW_BYTES + 8).map_err(|_| Error::Dma)?;
        let mut panel = Self {
            frame,
            history,
            _psram: psram,
            bus,
            scanout: Some(scanout),
            dma: Some(dma),
            control: 0,
        };
        // Preserve port 0, which controls the LoRa/GNSS supply.
        // Set output values before the expander enables those outputs.
        panel.control = panel.read(EXPANDER, 3)? & !DISPLAY_OUTPUTS;
        panel.write(EXPANDER, 3, panel.control)?;
        let direction = panel.read(EXPANDER, 7)?;
        panel.write(EXPANDER, 7, (direction & !DISPLAY_OUTPUTS) | 0xc4)?;
        let inversion = panel.read(EXPANDER, 5)?;
        panel.write(EXPANDER, 5, inversion & !PG)?;
        info!("display: ED047TC1 bus initialized; native=960x540; logical=540x960");
        Ok(panel)
    }

    fn read(&mut self, device: u8, register: u8) -> Result<u8, Error> {
        let mut result = [0];
        self.lock_bus()?
            .write_read(device, &[register], &mut result)
            .map_err(Error::I2c)?;
        Ok(result[0])
    }

    fn write(&mut self, device: u8, register: u8, value: u8) -> Result<(), Error> {
        self.lock_bus()?
            .write(device, &[register, value])
            .map_err(Error::I2c)
    }

    fn lock_bus(&self) -> Result<crate::bus::BusGuard<'_>, Error> {
        let deadline = Instant::now() + Duration::from_millis(100);
        self.bus
            .lock_blocking(|| {
                if Instant::now() >= deadline {
                    return false;
                }
                Delay::new().delay_micros(10);
                true
            })
            .ok_or(Error::Timeout("I2C bus ownership"))
    }

    fn set_control(&mut self, mask: u8, value: u8) -> Result<(), Error> {
        self.control = (self.control & !mask) | (value & mask);
        self.write(EXPANDER, 3, self.control)
    }

    fn wait_register(
        &mut self,
        device: u8,
        register: u8,
        mask: u8,
        value: u8,
        stage: &'static str,
    ) -> Result<(), Error> {
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            if self.read(device, register)? & mask == value {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(Error::Timeout(stage));
            }
            Delay::new().delay_millis(1);
        }
    }

    fn power_on(&mut self, io: &mut Scanout<'_>) -> Result<(), Error> {
        io.start_vertical.set_high();
        self.set_control(DISPLAY_OUTPUTS, WAKE | OE)?;
        Delay::new().delay_millis(3);
        let revision = self.read(SUPPLY, 0x10)?;
        self.write(SUPPLY, 0x0d, 0x80)?;
        self.wait_register(SUPPLY, 0x0d, 0x20, 0x20, "temperature")?;
        let temperature = self.read(SUPPLY, 0)? as i8;
        if !(0..=50).contains(&temperature) {
            return Err(Error::Temperature(temperature));
        }
        self.set_control(PWRUP, PWRUP)?;
        self.set_control(VCOM, VCOM)?;
        self.wait_register(EXPANDER, 1, PG, PG, "PWRGOOD")?;
        self.write(SUPPLY, 1, 0x3f)?;
        // The vendor H752-01 profile specifies -1.60 V. Do not program EEPROM.
        self.write(SUPPLY, 4, 0)?;
        self.write(SUPPLY, 3, 160)?;
        self.wait_register(SUPPLY, 0x0f, 0xfa, 0xfa, "TPS65185 rails")?;
        info!(
            "display: TPS65185 revision=0x{:02x}; temperature={} C; rails=0xfa",
            revision, temperature
        );
        Ok(())
    }

    fn power_off(&mut self, io: &mut Scanout<'_>) -> Result<(), Error> {
        let disable = self.set_control(OE | MODE | VCOM | PWRUP, 0);
        let discharge = self.wait_register(EXPANDER, 1, PG, 0, "power off");
        // Attempt shutdown even when the previous operation failed.
        let sleep = self.set_control(DISPLAY_OUTPUTS, 0);
        io.start_vertical.set_low();
        io.latch.set_low();
        disable.and(discharge).and(sleep)
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
        self.pulse(io, 10, 10)
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
        io.latch.set_high();
        Delay::new().delay_micros(1);
        io.latch.set_low();
        Delay::new().delay_micros(1);
        let data = [
            PulseCode::new(Level::High, ticks, Level::Low, 50),
            PulseCode::end_marker(),
        ];
        let pulse = io
            .clock_vertical
            .reborrow()
            .transmit(&data)
            .map_err(|_| Error::Rmt)?;
        let lcd = io.lcd.take().ok_or(Error::Dma)?;
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
        // Restore a white baseline before each grayscale image.
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
        let mut resources = self.scanout.take().ok_or(Error::Busy)?;
        let result = resources.open().and_then(|mut io| {
            info!("display: scanout drivers ready");
            let result = self.power_on(&mut io).and_then(|()| match update {
                Update::Full => self.update_full(&mut io),
                Update::Partial(_) => self.update_partial(&mut io),
                Update::Unchanged => unreachable!(),
            });
            let power_off = self.power_off(&mut io);
            if let Err(error) = &power_off {
                error!("display: power off failed: {}", error);
            }
            result.and(power_off)
        });
        // Dropping the temporary drivers releases both clock locks on success or error.
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
