#![no_std]
#![no_main]

extern crate alloc;

mod bus;
mod discovery;
mod display;
mod display_worker;
mod gnss;
mod power;
mod rtc;
#[cfg(feature = "radio-scan-demo")]
mod scan_demo;
mod touch;

use embassy_executor::Spawner;
use embassy_futures::select::{select, select4};
use embassy_time::{Instant, Timer};
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, system::Stack, timer::timg::TimerGroup};
use log::{error, info};
use paper_mesh_app::App;
use paper_mesh_platform::{Clock, location::Location, rotation::PortraitDisplay};

use rtc::FirmwareClock;
use static_cell::{ConstStaticCell, StaticCell};

static APP: ConstStaticCell<App> = ConstStaticCell::new(App::new_at(0));
static DISPLAY_STACK: ConstStaticCell<Stack<8192>> = ConstStaticCell::new(Stack::new());
static DISPLAY_EXECUTOR: StaticCell<esp_rtos::embassy::Executor> = StaticCell::new();

// This metadata describes the ESP image format.
// It does not add an ESP-IDF application runtime.
esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let panel = initialize(spawner);
    run_app(panel).await
}

fn initialize(spawner: Spawner) -> Option<display_worker::Client> {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::_240MHz);
    let peripherals = esp_hal::init(config);
    esp_println::logger::init_logger_from_env();

    // Radio drivers and RTOS tasks use internal RAM.
    esp_alloc::heap_allocator!(size: 128 * 1024);
    let timers = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timers.timer0, peripherals.FROM_CPU_INTR0);
    spawner.spawn(discovery::worker(peripherals.WIFI, peripherals.BT).unwrap());

    info!(
        "paper-mesh-a {}: T5 S3 E-Paper Pro display; core 0, CPU 240 MHz",
        env!("CARGO_PKG_VERSION")
    );

    let bus = bus::initialize(peripherals.I2C0, peripherals.GPIO39, peripherals.GPIO40)
        .map_err(|error| {
            error!("I2C: initialization failed: {:?}", error);
            gnss::fault();
            power::fault();
        })
        .ok()?;
    gnss::start(
        bus,
        peripherals.UART1,
        peripherals.GPIO44,
        peripherals.GPIO43,
        peripherals.FROM_CPU_INTR3,
    );
    spawner.spawn(rtc::worker(bus).unwrap());
    spawner.spawn(power::worker(bus).unwrap());
    let panel = display::Panel::new(
        display::Pins {
            data0: peripherals.GPIO5,
            data1: peripherals.GPIO6,
            data2: peripherals.GPIO7,
            data3: peripherals.GPIO15,
            data4: peripherals.GPIO16,
            data5: peripherals.GPIO17,
            data6: peripherals.GPIO18,
            data7: peripherals.GPIO8,
            latch: peripherals.GPIO42,
            start_horizontal: peripherals.GPIO41,
            clock_horizontal: peripherals.GPIO4,
            clock_vertical: peripherals.GPIO48,
            start_vertical: peripherals.GPIO45,
        },
        peripherals.PSRAM,
        bus,
        peripherals.LCD_CAM,
        peripherals.DMA_CH0,
        peripherals.RMT,
    )
    .map_err(|error| error!("display: initialization failed: {}", error))
    .ok();
    touch::start(
        bus,
        peripherals.GPIO9,
        peripherals.GPIO3,
        peripherals.IO_MUX,
        peripherals.FROM_CPU_INTR2,
    );
    esp_rtos::start_second_core(
        peripherals.CPU_CTRL,
        peripherals.FROM_CPU_INTR1,
        DISPLAY_STACK.take(),
        || {
            DISPLAY_EXECUTOR
                .init(esp_rtos::embassy::Executor::new())
                .run(|spawner| spawner.spawn(display_worker::worker().unwrap()));
        },
    );
    panel.map(display_worker::Client::new)
}

async fn run_app(mut panel: Option<display_worker::Client>) -> ! {
    let clock = FirmwareClock;
    // The application and its scan results use static RAM instead of the task stack.
    let app = APP.take();
    let mut discovery = discovery::FirmwareDiscovery;
    let mut location = gnss::FirmwareLocation;
    let mut power = power::FirmwarePower;
    #[cfg(feature = "radio-scan-demo")]
    scan_demo::run(&mut discovery).await;
    loop {
        app.poll_discovery(&clock, &mut discovery);
        app.poll_location(&clock, &mut location);
        app.poll_power(&clock, &mut power);
        while let Some(sample) = touch::next_event() {
            info!(
                "touch: {:?}; irq_ms={}; service_us={}; bus_wait_us={}; refresh={}",
                sample.event,
                sample.irq_ms,
                sample.service_us,
                sample.bus_wait_us,
                sample.during_refresh
            );
            let previous = app.taps();
            app.touch(sample.event);
            if app.taps() != previous {
                info!(
                    "touch: count={}; stats={:?}",
                    app.taps(),
                    touch::statistics()
                );
            }
        }
        if let Some(display) = panel.as_mut()
            && let Err(error) = display.complete().and_then(|()| {
                let submitted = app.present(&mut PortraitDisplay::new(display))?;
                if submitted {
                    info!("display: submitted; count={}", app.taps());
                }
                Ok::<(), display::Error>(())
            })
        {
            error!("display: refresh failed: {}; updates stopped", error);
            panel = None;
        }
        select4(
            touch::changed(),
            display_worker::changed(),
            select(
                discovery::changed(),
                select(gnss::changed(), power::changed()),
            ),
            Timer::at(Instant::from_millis(app.next_deadline_ms())),
        )
        .await;
        if let Some(uptime) = app.tick(&clock) {
            info!("heartbeat: uptime={}s; utc={:?}", uptime, clock.now_utc());
            info!("touch: stats={:?}", touch::statistics());
            let snapshot = location.snapshot(clock.now_ms());
            info!(
                "GNSS: messages={}; response_age_ms={:?}; in_view={:?}; with_signal={:?}; fix={}",
                snapshot
                    .activity
                    .map_or(0, |activity| activity.valid_messages),
                snapshot
                    .activity
                    .map(|activity| clock.now_ms().saturating_sub(activity.received_at_ms)),
                snapshot
                    .satellites
                    .map(|satellites| satellites.satellites_in_view),
                snapshot
                    .satellites
                    .map(|satellites| satellites.satellites_with_signal),
                snapshot
                    .observation
                    .is_some_and(|observation| observation.coordinates.is_some()
                        && clock.now_ms().saturating_sub(observation.observed_at_ms) < 5_000),
            );
        }
    }
}
