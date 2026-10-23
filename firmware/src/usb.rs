//! Composite USB transport. HID input and diagnostic CDC share the internal PHY.

use core::{
    fmt::Write,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, signal::Signal,
};
use embassy_time::{Duration, Instant, Timer, with_timeout};
use embassy_usb::{
    Builder, Config, Handler, UsbDevice,
    class::{cdc_acm, hid},
    control::OutResponse,
};
use esp_hal::{
    peripherals::{GPIO19, GPIO20, USB_FS},
    usb::otg::{
        Usb,
        embassy_usb_device::{Config as DriverConfig, Driver},
    },
};
use paper_mesh_hid_touchpad::{Descriptor, Encoder, Geometry, REPORT_LEN};
use paper_mesh_platform::touchpad::{HostFrame, HostState, TouchpadHost};
use static_cell::StaticCell;

static DESCRIPTOR: Descriptor = Descriptor::new(Geometry {
    width: 540,
    height: 960,
    width_tenth_mm: 583,
    height_tenth_mm: 1037,
});
static OUT_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();
static CONFIG_BUFFER: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_BUFFER: StaticCell<[u8; 64]> = StaticCell::new();
static CONTROL_BUFFER: StaticCell<[u8; 512]> = StaticCell::new();
static CDC: StaticCell<cdc_acm::State<'static>> = StaticCell::new();
static HID: StaticCell<hid::State<'static>> = StaticCell::new();
static EVENTS: StaticCell<Events> = StaticCell::new();
static REQUESTS: StaticCell<Requests> = StaticCell::new();
static STATUS: AtomicU32 = AtomicU32::new(0);
static GENERATION: AtomicU32 = AtomicU32::new(0);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static RESTART: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static FRAMES: Channel<CriticalSectionRawMutex, (u32, HostFrame), 16> = Channel::new();
static LOGS: Channel<CriticalSectionRawMutex, heapless::String<192>, 32> = Channel::new();
static LOGGER: Logger = Logger;

pub struct FirmwareTouchpad;
impl TouchpadHost for FirmwareTouchpad {
    fn state(&self) -> HostState {
        match STATUS.load(Ordering::Acquire) {
            1 => HostState::Ready,
            2 => HostState::Suspended,
            3 => HostState::Fault,
            _ => HostState::Disconnected,
        }
    }
    fn generation(&self) -> u32 {
        GENERATION.load(Ordering::Acquire)
    }
    fn set_active(&mut self, active: bool) {
        let previous = ACTIVE.swap(active, Ordering::AcqRel);
        if previous != active {
            log::info!("touchpad: active={}; host={:?}", active, self.state());
        }
        if previous && !active {
            self.cancel();
        }
    }
    fn cancel(&mut self) {
        log::info!("touchpad: cancel; host={:?}", self.state());
        change(0);
        RESTART.signal(());
    }
    fn submit(&mut self, frame: HostFrame) -> bool {
        ACTIVE.load(Ordering::Acquire)
            && self.state() == HostState::Ready
            && FRAMES.try_send((self.generation(), frame)).is_ok()
    }
}

pub async fn changed() {
    CHANGED.wait().await;
}
fn change(status: u32) {
    STATUS.store(status, Ordering::Release);
    GENERATION.fetch_add(1, Ordering::AcqRel);
    FRAMES.clear();
    CHANGED.signal(());
}
struct Events;
impl Handler for Events {
    fn enabled(&mut self, enabled: bool) {
        if !enabled {
            change(0);
        }
    }
    fn reset(&mut self) {
        change(0);
    }
    fn configured(&mut self, configured: bool) {
        log::info!("USB: configured={}", configured);
        change(u32::from(configured));
    }
    fn suspended(&mut self, suspended: bool) {
        if suspended && STATUS.load(Ordering::Acquire) == 1 {
            change(2);
        } else if !suspended && STATUS.load(Ordering::Acquire) == 2 {
            // Detach after resume so Linux cannot retain an interrupted gesture.
            FirmwareTouchpad.cancel();
        }
    }
}
struct Requests;
impl hid::RequestHandler for Requests {
    fn get_report(&mut self, id: hid::ReportId, data: &mut [u8]) -> Option<usize> {
        if let hid::ReportId::Feature(id) = id {
            paper_mesh_hid_touchpad::get_feature(id, data)
        } else {
            None
        }
    }
    fn set_report(&mut self, id: hid::ReportId, data: &[u8]) -> OutResponse {
        if let hid::ReportId::Feature(id) = id
            && paper_mesh_hid_touchpad::set_feature(id, data)
        {
            return OutResponse::Accepted;
        }
        OutResponse::Rejected
    }
}

pub fn start(
    spawner: Spawner,
    peripheral: USB_FS<'static>,
    dp: GPIO20<'static>,
    dm: GPIO19<'static>,
) {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);
    let driver = Driver::new(
        Usb::new_fs(peripheral, dp, dm),
        OUT_BUFFER.init([0; 256]),
        DriverConfig::default(),
    );
    // These IDs are for a private prototype. They are not allocated product IDs.
    let mut config = Config::new(0xcafe, 0x4010);
    config.manufacturer = Some("Paper Mesh");
    config.product = Some("Paper Mesh Touchpad");
    config.serial_number = Some("paper-mesh-prototype");
    config.self_powered = true;
    config.max_power = 500;
    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_BUFFER.init([0; 256]),
        BOS_BUFFER.init([0; 64]),
        &mut [],
        CONTROL_BUFFER.init([0; 512]),
    );
    builder.handler(EVENTS.init(Events));
    let cdc = cdc_acm::CdcAcmClass::new(&mut builder, CDC.init(cdc_acm::State::new()), 64);
    let hid = hid::HidWriter::<_, REPORT_LEN>::new(
        &mut builder,
        HID.init(hid::State::new()),
        hid::Config {
            report_descriptor: DESCRIPTOR.as_bytes(),
            request_handler: Some(REQUESTS.init(Requests)),
            poll_ms: 8,
            max_packet_size: 64,
            hid_subclass: hid::HidSubclass::No,
            hid_boot_protocol: hid::HidBootProtocol::None,
        },
    );
    let (sender, _) = cdc.split();
    spawner.spawn(bus_worker(builder.build()).unwrap());
    spawner.spawn(hid_worker(hid).unwrap());
    spawner.spawn(log_worker(sender).unwrap());
}

#[embassy_executor::task]
async fn bus_worker(mut device: UsbDevice<'static, Driver<'static>>) {
    use crate::suspend::{self, Service};
    suspend::register(Service::Usb);
    loop {
        select(
            device.run(),
            select(RESTART.wait(), suspend::request(Service::Usb)),
        )
        .await;
        // The resolved Synopsys driver's disable() does not disconnect the PHY.
        // Keep clocks running and detach before resetting the USB stack state.
        USB_FS::regs().pcgcctl().modify(|_, w| {
            w.stoppclk().clear_bit();
            w.gatehclk().clear_bit()
        });
        USB_FS::regs().dctl().modify(|_, w| w.sftdiscon().set_bit());
        // Embassy requires disable after cancellation of run().
        device.disable().await;
        suspend::checkpoint(Service::Usb).await;
        Timer::after_millis(250).await;
        USB_FS::regs()
            .dctl()
            .modify(|_, w| w.sftdiscon().clear_bit());
        log::info!("USB: reconnect");
    }
}

#[embassy_executor::task]
async fn hid_worker(mut writer: hid::HidWriter<'static, Driver<'static>, REPORT_LEN>) {
    use crate::suspend::{self, Service};
    suspend::register(Service::Hid);
    loop {
        suspend::checkpoint(Service::Hid).await;
        select(suspend::request(Service::Hid), write_hid(&mut writer)).await;
    }
}

async fn write_hid(writer: &mut hid::HidWriter<'static, Driver<'static>, REPORT_LEN>) {
    let mut generation = 0;
    let mut encoder = Encoder::default();
    let mut held: Option<(u32, HostFrame)> = None;
    loop {
        let (epoch, frame) = if let Some((epoch, mut frame)) = held {
            match select(FRAMES.receive(), Timer::after_millis(16)).await {
                Either::First(next) => next,
                Either::Second(()) => {
                    // Linux releases Win8 contacts after 100 ms without a report.
                    frame.at_ms = Instant::now().as_millis();
                    (epoch, frame)
                }
            }
        } else {
            FRAMES.receive().await
        };
        held = None;
        if epoch != GENERATION.load(Ordering::Acquire) || !ACTIVE.load(Ordering::Acquire) {
            continue;
        }
        if epoch != generation {
            encoder = Encoder::default();
            generation = epoch;
            log::info!("touchpad: first report; generation={}", generation);
        }
        let reports = encoder.encode(frame);
        for packet in reports.packets() {
            if epoch != GENERATION.load(Ordering::Acquire) {
                break;
            }
            if !matches!(
                with_timeout(Duration::from_millis(100), writer.write(packet)).await,
                Ok(Ok(()))
            ) {
                if epoch == GENERATION.load(Ordering::Acquire) {
                    log::warn!("touchpad: HID write failed or timed out");
                    FirmwareTouchpad.cancel();
                }
                break;
            }
        }
        if epoch == GENERATION.load(Ordering::Acquire) && !frame.contacts.is_empty() {
            held = Some((epoch, frame));
        }
    }
}

struct Logger;
impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            let mut line = heapless::String::new();
            let _ = writeln!(line, "{} {}", record.level(), record.args());
            let _ = LOGS.try_send(line);
        }
    }
    fn flush(&self) {}
}

#[embassy_executor::task]
async fn log_worker(mut sender: cdc_acm::Sender<'static, Driver<'static>>) {
    use crate::suspend::{self, Service};
    suspend::register(Service::Logs);
    loop {
        suspend::checkpoint(Service::Logs).await;
        select(suspend::request(Service::Logs), write_logs(&mut sender)).await;
    }
}

async fn write_logs(sender: &mut cdc_acm::Sender<'static, Driver<'static>>) {
    let mut connection = None;
    loop {
        let generation = GENERATION.load(Ordering::Acquire);
        // The driver's disable leaves endpoints enabled until the next USB reset.
        // Wait for a new configuration before accessing those stale endpoints.
        if !log_connection(sender, generation) {
            connection = None;
            Timer::after_millis(50).await;
            continue;
        }
        sender.wait_connection().await;
        if connection != Some(generation) {
            if send_log_line(
                sender,
                &crate::suspend::trace::connection_line(),
                generation,
            )
            .await
                && send_log_line(sender, &crate::suspend::trace::wake_line(), generation).await
                && send_log_line(
                    sender,
                    &crate::display_recovery::connection_line(),
                    generation,
                )
                .await
            {
                connection = Some(generation);
            }
            continue;
        }
        let line = match select(LOGS.receive(), Timer::after_millis(250)).await {
            Either::First(line) => line,
            Either::Second(()) => continue,
        };
        if !send_log_line(sender, &line, generation).await {
            connection = None;
        }
    }
}

fn log_connection(sender: &cdc_acm::Sender<'static, Driver<'static>>, generation: u32) -> bool {
    STATUS.load(Ordering::Acquire) == 1
        && GENERATION.load(Ordering::Acquire) == generation
        && sender.dtr()
}

async fn send_log_line(
    sender: &mut cdc_acm::Sender<'static, Driver<'static>>,
    line: &str,
    generation: u32,
) -> bool {
    if line.is_empty() {
        return true;
    }
    let packets = line
        .as_bytes()
        .chunks(64)
        .chain(line.len().is_multiple_of(64).then_some(&[][..]));
    for packet in packets {
        if !log_connection(sender, generation)
            || !matches!(
                with_timeout(Duration::from_millis(250), sender.write_packet(packet)).await,
                Ok(Ok(()))
            )
        {
            return false;
        }
    }
    true
}
