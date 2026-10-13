#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Instant, Timer};
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, timer::timg::TimerGroup};
use log::info;
use paper_mesh_app::{App, platform::Clock};

struct FirmwareClock;

impl Clock for FirmwareClock {
    fn now_ms(&self) -> u64 {
        Instant::now().as_millis()
    }
}

// Metadata for the ESP image format, not an ESP-IDF application runtime.
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::_80MHz);
    let peripherals = esp_hal::init(config);
    esp_println::logger::init_logger_from_env();

    let timers = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timers.timer0, peripherals.FROM_CPU_INTR0);

    info!(
        "paper-mesh-a {}: T5 S3 E-Paper Pro bootstrap; core 0, CPU 80 MHz",
        env!("CARGO_PKG_VERSION")
    );

    // Board peripherals stay uninitialized until their drivers and power
    // sequencing have been verified. In particular, do not transmit on LoRa.
    // Only the shared application's clock is connected on hardware so far.
    let clock = FirmwareClock;
    let mut app = App::new(&clock);
    loop {
        Timer::at(Instant::from_millis(app.next_deadline_ms())).await;
        if let Some(uptime) = app.tick(&clock) {
            info!("heartbeat: uptime={}s", uptime);
        }
    }
}
