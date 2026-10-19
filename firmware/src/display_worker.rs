//! Exclusive panel ownership passes between the runtime and core 1.

use crate::display::NATIVE_SCREEN_SIZE;
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    channel::{Channel, TrySendError},
    signal::Signal,
};
use embedded_graphics::{pixelcolor::Gray4, prelude::*};
use esp_hal::system::Cpu;
use log::info;
use paper_mesh_platform::Display;

use crate::display::{Error, Panel};

static REQUEST: Channel<CriticalSectionRawMutex, Panel, 1> = Channel::new();
static COMPLETE: Channel<CriticalSectionRawMutex, (Panel, Result<(), Error>), 1> = Channel::new();
static CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static REFRESHING: AtomicBool = AtomicBool::new(false);

pub fn is_refreshing() -> bool {
    REFRESHING.load(Ordering::Relaxed)
}

pub async fn changed() {
    CHANGED.wait().await;
}

#[embassy_executor::task]
pub async fn worker() {
    assert!(Cpu::current() == Cpu::AppCpu);
    info!("display: worker started; core=1");
    loop {
        let mut panel = REQUEST.receive().await;
        info!("display: refresh started");
        REFRESHING.store(true, Ordering::Relaxed);
        let result = panel.refresh();
        REFRESHING.store(false, Ordering::Relaxed);
        // The runtime cannot submit again before it receives this panel.
        assert!(COMPLETE.try_send((panel, result)).is_ok());
        CHANGED.signal(());
    }
}

pub struct Client {
    panel: Option<Panel>,
}

impl Client {
    pub fn new(panel: Panel) -> Self {
        Self { panel: Some(panel) }
    }

    /// Return ownership after completion. Report failures to platform supervision.
    pub fn complete(&mut self) -> Result<(), Error> {
        if let Ok((panel, result)) = COMPLETE.try_receive() {
            assert!(self.panel.is_none());
            self.panel = Some(panel);
            result?;
        }
        Ok(())
    }
}

impl OriginDimensions for Client {
    fn size(&self) -> Size {
        NATIVE_SCREEN_SIZE
    }
}

impl DrawTarget for Client {
    type Color = Gray4;
    type Error = Error;

    fn draw_iter<I: IntoIterator<Item = Pixel<Gray4>>>(&mut self, pixels: I) -> Result<(), Error> {
        self.panel.as_mut().ok_or(Error::Busy)?.draw_iter(pixels)
    }

    fn clear(&mut self, color: Gray4) -> Result<(), Error> {
        self.panel.as_mut().ok_or(Error::Busy)?.clear(color)
    }
}

impl Display for Client {
    fn is_busy(&self) -> bool {
        self.panel.is_none()
    }

    fn refresh(&mut self) -> Result<(), Error> {
        let panel = self.panel.take().ok_or(Error::Busy)?;
        if let Err(TrySendError::Full(panel)) = REQUEST.try_send(panel) {
            self.panel = Some(panel);
            return Err(Error::Busy);
        }
        Ok(())
    }
}
