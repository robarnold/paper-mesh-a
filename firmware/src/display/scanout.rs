//! Peripheral ownership and temporary drivers for one panel refresh.

use esp_hal::{
    Blocking,
    gpio::{Level, Output, OutputConfig},
    lcd_cam::{
        LcdCam,
        lcd::i8080::{self, I8080},
    },
    peripherals,
    rmt::{self, Channel, Tx, TxChannelCreator},
    time::Rate,
};

use super::{Error, Pins};

impl Pins {
    pub(super) fn idle(&mut self) {
        // Explicit GPIO routing survives driver release and peripheral clock gating.
        macro_rules! low {
            ($($pin:ident),+ $(,)?) => {
                $(let _pin = Output::new(self.$pin.reborrow(), Level::Low, OutputConfig::default());)+
            };
        }
        low!(
            data0,
            data1,
            data2,
            data3,
            data4,
            data5,
            data6,
            data7,
            latch,
            start_horizontal,
            clock_horizontal,
            clock_vertical,
            start_vertical
        );
    }
}

pub struct Resources {
    pins: Pins,
    lcd: peripherals::LCD_CAM<'static>,
    dma: peripherals::DMA_CH0<'static>,
    rmt: peripherals::RMT<'static>,
}

pub struct Scanout<'a> {
    pub latch: Output<'a>,
    pub start_vertical: Output<'a>,
    pub clock_vertical: Channel<'a, Blocking, Tx>,
    pub lcd: Option<I8080<'a, Blocking>>,
}

impl Resources {
    pub fn idle(&mut self) {
        self.pins.idle();
    }

    pub fn new(
        pins: Pins,
        lcd: peripherals::LCD_CAM<'static>,
        dma: peripherals::DMA_CH0<'static>,
        rmt: peripherals::RMT<'static>,
    ) -> Self {
        Self {
            pins,
            lcd,
            dma,
            rmt,
        }
    }

    /// Borrow the owned peripherals until the refresh and rail shutdown finish.
    pub fn open(&mut self) -> Result<Scanout<'_>, Error> {
        let lcd = LcdCam::new(self.lcd.reborrow());
        let lcd = I8080::new(
            lcd.lcd,
            self.dma.reborrow(),
            i8080::Config::default()
                .with_frequency(Rate::from_mhz(20))
                .with_cd_idle_edge(false)
                .with_cd_cmd_edge(true)
                .with_cd_dummy_edge(false)
                .with_cd_data_edge(false),
        )
        .map_err(|_| Error::Configuration)?
        .with_dc(self.pins.start_horizontal.reborrow())
        .with_wrx(self.pins.clock_horizontal.reborrow())
        .with_data0(self.pins.data6.reborrow())
        .with_data1(self.pins.data7.reborrow())
        .with_data2(self.pins.data4.reborrow())
        .with_data3(self.pins.data5.reborrow())
        .with_data4(self.pins.data2.reborrow())
        .with_data5(self.pins.data3.reborrow())
        .with_data6(self.pins.data0.reborrow())
        .with_data7(self.pins.data1.reborrow());
        let rmt = rmt::Rmt::new(self.rmt.reborrow(), Rate::from_mhz(80))
            .map_err(|_| Error::Configuration)?;
        let clock_vertical = rmt
            .channel1
            .configure_tx(
                &rmt::TxChannelConfig::default()
                    .with_clk_divider(8)
                    .with_idle_output_level(Level::Low)
                    .with_idle_output(true),
            )
            .map_err(|_| Error::Configuration)?
            .with_pin(self.pins.clock_vertical.reborrow());
        Ok(Scanout {
            lcd: Some(lcd),
            clock_vertical,
            latch: Output::new(
                self.pins.latch.reborrow(),
                Level::Low,
                OutputConfig::default(),
            ),
            start_vertical: Output::new(
                self.pins.start_vertical.reborrow(),
                Level::Low,
                OutputConfig::default(),
            ),
        })
    }
}
