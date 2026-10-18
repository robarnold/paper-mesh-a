//! Shared I2C access for display controls and touch reports.

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use esp_hal::{
    Blocking,
    i2c::master::{Config, I2c, SoftwareTimeout},
    peripherals,
    time::{Duration, Rate},
};
use static_cell::StaticCell;

// The mutex masks interrupts only while it changes its lock state.
// A guard keeps the bus exclusive while interrupts remain enabled.
pub type Bus = Mutex<CriticalSectionRawMutex, I2c<'static, Blocking>>;
static BUS: StaticCell<Bus> = StaticCell::new();

pub fn initialize(
    i2c: peripherals::I2C0<'static>,
    sda: peripherals::GPIO39<'static>,
    scl: peripherals::GPIO40<'static>,
) -> Result<&'static Bus, esp_hal::i2c::master::ConfigError> {
    let driver = I2c::new(
        i2c,
        Config::default()
            .with_frequency(Rate::from_khz(100))
            .with_software_timeout(SoftwareTimeout::Transaction(Duration::from_millis(20))),
    )?
    .with_sda(sda)
    .with_scl(scl);
    Ok(BUS.init(Mutex::new(driver)))
}
