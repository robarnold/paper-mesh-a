#![no_std]
#![no_main]

extern crate alloc;

mod bus;
mod buttons;

mod diagnostics;
mod discovery;
mod display;

mod display_recovery;

mod display_worker;
mod gnss;
mod power;
mod recovery;
mod rtc;
#[cfg(feature = "radio-scan-demo")]
mod scan_demo;
mod suspend;
mod touch;
mod usb;

use embassy_executor::Spawner;
use embassy_futures::select::{select, select4};
use embassy_time::{Duration, Instant, Timer, with_timeout};
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, system::Stack, timer::timg::TimerGroup};
use log::{error, info};
use paper_mesh_app::App;
use paper_mesh_platform::touchpad::{TouchpadHost, TouchpadSession};
use paper_mesh_platform::{Clock, Display, location::Location, rotation::PortraitDisplay};

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
    let (panel, bus, suspend) = initialize(spawner);
    run_app(panel, bus, suspend).await
}

fn initialize(
    spawner: Spawner,
) -> (
    Option<display_worker::Client>,
    Option<&'static bus::Bus>,
    suspend::Controller,
) {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::_240MHz);
    let peripherals = esp_hal::init(config);
    let suspend = suspend::initialize(peripherals.GPIO38, peripherals.LPWR, peripherals.RTC_TIMER);
    display_recovery::boot();
    assert!(recovery::arm(recovery::Owner::Boot, 15_000));
    let bus_result = bus::initialize(peripherals.I2C0, peripherals.GPIO39, peripherals.GPIO40);
    let mut pins = display::Pins {
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
    };
    // External expander outputs survive an ESP reset. Stop them before PSRAM setup.
    let display_safe = bus_result
        .as_ref()
        .ok()
        .map(|bus| display::establish_safe_state(bus, &mut pins));
    if matches!(display_safe, Some(Ok(()))) {
        recovery::disable(recovery::Owner::Boot);
    } else {
        display_recovery::latch_fault(display_recovery::Stage::ShuttingDown);
    }

    // Radio drivers and RTOS tasks use internal RAM.
    esp_alloc::heap_allocator!(size: 128 * 1024);
    let timers = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timers.timer0, peripherals.FROM_CPU_INTR0);
    usb::start(
        spawner,
        peripherals.USB_FS,
        peripherals.GPIO20,
        peripherals.GPIO19,
    );
    diagnostics::report_previous_panic();
    display_recovery::report_previous();
    suspend::trace::boot();
    spawner.spawn(discovery::worker(peripherals.WIFI, peripherals.BT).unwrap());

    info!(
        "paper-mesh-a {}: T5 S3 E-Paper Pro display; core 0, CPU 240 MHz",
        env!("CARGO_PKG_VERSION")
    );

    let bus = bus_result
        .map_err(|error| {
            error!("I2C: initialization failed: {:?}", error);
            gnss::fault();
            power::fault();
        })
        .ok();
    spawner.spawn(buttons::worker(bus, peripherals.GPIO0).unwrap());
    let Some(bus) = bus else {
        return (None, None, suspend);
    };
    gnss::start(
        bus,
        peripherals.UART1,
        peripherals.GPIO44,
        peripherals.GPIO43,
        peripherals.FROM_CPU_INTR3,
    );
    spawner.spawn(rtc::worker(bus).unwrap());
    spawner.spawn(power::worker(bus).unwrap());
    let panel = if display_safe.is_some_and(|result| {
        if let Err(error) = &result {
            error!("display: boot shutdown failed: {}", error);
        }
        result.is_ok()
    }) && !display_recovery::blocked()
    {
        display::Panel::new(
            pins,
            peripherals.PSRAM,
            bus,
            peripherals.LCD_CAM,
            peripherals.DMA_CH0,
            peripherals.RMT,
        )
        .map_err(|error| error!("display: initialization failed: {}", error))
        .ok()
    } else {
        error!("display: updates blocked; disconnect USB and hold BOOT for 3 seconds to power off");
        None
    };
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
    (panel.map(display_worker::Client::new), Some(bus), suspend)
}

async fn run_app(
    mut panel: Option<display_worker::Client>,
    bus: Option<&'static bus::Bus>,
    mut suspend: suspend::Controller,
) -> ! {
    let clock = FirmwareClock;
    // The application and its scan results use static RAM instead of the task stack.
    let app = APP.take();
    app.set_suspend_controls(&buttons::CONTROLS[..2]);
    app.set_power_off_restart_hint("Hold Power for 2 seconds or connect USB.");
    app.set_power_off_control(buttons::BOOT);
    let mut discovery = discovery::FirmwareDiscovery;
    let mut location = gnss::FirmwareLocation;
    let mut power = power::FirmwarePower;
    let mut touchpad = usb::FirmwareTouchpad;
    let mut touchpad_session = TouchpadSession::new();
    #[cfg(feature = "radio-scan-demo")]
    scan_demo::run(&mut discovery).await;
    loop {
        touchpad_session.set_active(app.touchpad_open(), &mut touchpad);
        touchpad_session.poll(&touchpad);
        app.set_touchpad_host(touchpad.state());
        app.poll_discovery(&clock, &mut discovery);
        app.poll_location(&clock, &mut location);
        app.poll_power(&clock, &mut power);
        process_touch(app, &mut touchpad_session, &mut touchpad);
        // Drain earlier touch frames before applying an exit button.
        while let Some(event) = buttons::next_event() {
            info!("button: {:?}", event);
            app.hardware_button(event, &[buttons::FUNCTION, buttons::HOME]);
        }
        touchpad_session.set_active(app.touchpad_open(), &mut touchpad);
        if app.take_power_off_request() {
            app.cancel_touchpad();
            touchpad_session.set_active(false, &mut touchpad);
            power_off_app(app, panel.as_mut(), bus, &mut suspend).await;
            continue;
        }
        if let Some(request) = app.take_suspend_request() {
            app.cancel_touchpad();
            touchpad_session.set_active(false, &mut touchpad);
            alloc::boxed::Box::pin(suspend_app(app, panel.as_mut(), bus, request, &mut suspend))
                .await;
            continue;
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
            select(touch::changed(), select(buttons::changed(), usb::changed())),
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

fn process_touch(app: &mut App, session: &mut TouchpadSession, host: &mut usb::FirmwareTouchpad) {
    while let Some(sample) = touch::next_event() {
        log::debug!(
            "touch: {:?}; irq_ms={}; service_us={}; bus_wait_us={}; refresh={}",
            sample.frame,
            sample.irq_ms,
            sample.service_us,
            sample.bus_wait_us,
            sample.during_refresh
        );
        let previous = app.taps();
        let was_touchpad_open = app.touchpad_open();
        if session.input(&sample.frame, host) {
            for event in sample.frame.events() {
                app.touch(event);
            }
        }
        session.set_active(app.touchpad_open(), host);
        if app.touchpad_open() != was_touchpad_open {
            info!("touchpad: application mode={}", app.touchpad_open());
        }
        if app.taps() != previous {
            info!(
                "touch: count={}; stats={:?}",
                app.taps(),
                touch::statistics()
            );
        }
    }
}

fn power_off_app<'a>(
    app: &'a mut App,
    panel: Option<&'a mut display_worker::Client>,
    bus: Option<&'a bus::Bus>,
    controller: &'a mut suspend::Controller,
) -> core::pin::Pin<alloc::boxed::Box<dyn core::future::Future<Output = ()> + 'a>> {
    alloc::boxed::Box::pin(power_off_app_inner(app, panel, bus, controller))
}

async fn power_off_app_inner(
    app: &mut App,
    panel: Option<&mut display_worker::Client>,
    bus: Option<&bus::Bus>,
    controller: &mut suspend::Controller,
) {
    use paper_mesh_platform::power_off::PowerOffError;
    let result = async {
        let bus = bus.ok_or(PowerOffError::Unsupported)?;
        // Reject external power before a final display update or service pause.
        with_timeout(Duration::from_secs(1), power::check_power_off(bus))
            .await
            .map_err(|_| PowerOffError::Preparation)??;
        if let Some(display) = panel {
            with_timeout(Duration::from_secs(30), async {
                loop {
                    if let Err(error) = display.complete() {
                        error!("power off: final display unavailable: {}", error);
                        // The coordinator must confirm controls off before any battery cut.
                        break Ok::<(), PowerOffError>(());
                    }
                    if !display.is_busy() {
                        match app.present(&mut PortraitDisplay::new(&mut *display)) {
                            Ok(false) => break Ok(()),
                            Ok(true) => {}
                            Err(error) => {
                                error!("power off: final display unavailable: {}", error);
                                break Ok(());
                            }
                        }
                    }
                    display_worker::changed().await;
                }
            })
            .await
            .map_err(|_| PowerOffError::Busy)??;
        }
        controller.power_off(bus).await
    }
    .await;
    // Successful battery isolation cannot return a firmware result.
    let error = result.err().unwrap_or(PowerOffError::StillPowered);
    error!("power off: {}", error.label());
    app.finish_power_off(error);
}

async fn suspend_app(
    app: &mut App,
    mut panel: Option<&mut display_worker::Client>,
    bus: Option<&bus::Bus>,
    request: paper_mesh_platform::suspend::SuspendRequest,
    suspend: &mut suspend::Controller,
) {
    let before_frame = if let Some(display) = panel.as_deref_mut() {
        with_timeout(Duration::from_secs(10), async {
            loop {
                display.complete()?;
                if !display.is_busy() && !app.present(&mut PortraitDisplay::new(&mut *display))? {
                    break display.frame_checksum();
                }
                display_worker::changed().await;
            }
        })
        .await
        .ok()
        .and_then(Result::ok)
    } else {
        None
    };
    let result = if before_frame.is_some()
        && let Some(bus) = bus
    {
        alloc::boxed::Box::pin(suspend.run(bus, request)).await
    } else {
        Err(paper_mesh_platform::suspend::SuspendError::Preparation)
    };
    if let (Some(display), Some(before)) = (panel, before_frame)
        && let Err(error) = display.resume_from_suspend(before)
    {
        error!("display: suspend restoration failed: {}", error);
    }
    match &result {
        Ok(report) => info!(
            "suspend: resumed; elapsed_ms={}; wake_reasons={:?}; charge_loss_mah={:?}",
            report.elapsed_ms,
            report.wake_reasons,
            report.charge_loss_mah()
        ),
        Err(error) => error!("suspend: failed: {:?}", error),
    }
    app.finish_suspend(result);
}
