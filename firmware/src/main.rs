#![no_std]
#![no_main]

extern crate alloc;

mod discovery;
#[cfg(feature = "radio-scan-demo")]
mod scan_demo;

use embassy_executor::Spawner;
use embassy_futures::select::select;
use embassy_time::{Instant, Timer};
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, timer::timg::TimerGroup};
use log::info;
use paper_mesh_app::{App, platform::Clock};
use static_cell::ConstStaticCell;

static APP: ConstStaticCell<App> = ConstStaticCell::new(App::new_at(0));

struct FirmwareClock;

impl Clock for FirmwareClock {
    fn now_ms(&self) -> u64 {
        Instant::now().as_millis()
    }
}

// This metadata describes the ESP image format.
// It does not add an ESP-IDF application runtime.
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    initialize(spawner);
    run_app().await
}

fn initialize(spawner: Spawner) {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::_80MHz);
    let peripherals = esp_hal::init(config);
    esp_println::logger::init_logger_from_env();

    // Radio drivers and RTOS tasks use internal RAM. PSRAM stays uninitialized.
    esp_alloc::heap_allocator!(size: 128 * 1024);
    let timers = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timers.timer0, peripherals.FROM_CPU_INTR0);
    spawner.spawn(discovery::worker(peripherals.WIFI, peripherals.BT).unwrap());

    info!(
        "paper-mesh-a {}: T5 S3 E-Paper Pro bootstrap; core 0, CPU 80 MHz",
        env!("CARGO_PKG_VERSION")
    );
}

async fn run_app() -> ! {
    // Peripheral initialization must use validated drivers and power sequences.
    // Do not transmit on LoRa.
    // Firmware supplies the clock and passive radio scans.
    let clock = FirmwareClock;
    // The application and its scan results use static RAM instead of the task stack.
    let app = APP.take();
    let mut discovery = discovery::FirmwareDiscovery;
    #[cfg(feature = "radio-scan-demo")]
    scan_demo::run(&mut discovery).await;
    loop {
        app.poll_discovery(&clock, &mut discovery);
        select(
            discovery::changed(),
            Timer::at(Instant::from_millis(app.next_deadline_ms())),
        )
        .await;
        if let Some(uptime) = app.tick(&clock) {
            info!("heartbeat: uptime={}s", uptime);
        }
    }
}
