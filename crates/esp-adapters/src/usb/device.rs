//! Composite USB transport. HID input and diagnostic CDC share the internal PHY.
//!
//! # Design choices
//!
//! - Expose touchpad reports and diagnostic logs through one composite device because
//!   both functions share the internal USB PHY.
//!
//! See [ESP USB adapter](../../../../docs/adapters/usb.md).

mod workers;
use super::{control, lifecycle::UsbLifecycle};
use paper_mesh_platform::worker::Pause;
pub use workers::UsbWorkers;
use workers::{CdcWorkers, DeviceWorker, HidWorker};

use paper_mesh_platform::transport::{ReplyTransport, SessionId, framing::FrameError};

/// Fixed command-buffer capacity. Runtime protocol limits must fit this storage.
pub const REQUEST_CAPACITY: usize = 512;

/// Runtime-selected dispatch for a complete request or framing error in its session.
pub type CommandHandler = fn(Result<&[u8], FrameError>, SessionId) -> Option<alloc::string::String>;

/// Supply one connection diagnostic without waiting or accessing USB.
pub type DiagnosticSource = fn() -> heapless::String<192>;

use core::{
    cell::RefCell,
    fmt::Write,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    channel::Channel,
    signal::Signal,
};
use embassy_time::{Duration, with_timeout};
use embassy_usb::{
    Builder, Handler,
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
use paper_mesh_hid_touchpad::{Descriptor, REPORT_LEN};
use paper_mesh_platform::touchpad::{HostFrame, HostState, TouchpadHost};
use static_cell::StaticCell;

/// Runtime-selected device identity, touchpad geometry, and command dispatch.
pub struct Config {
    pub device: embassy_usb::Config<'static>,
    /// Must describe the logical coordinates submitted through TouchpadHost.
    pub descriptor: &'static Descriptor,
    pub command_handler: CommandHandler,
    /// Maximum payload bytes, excluding LF. Must not exceed REQUEST_CAPACITY.
    pub max_request_len: usize,
    /// Called in order when diagnostic output starts for a configured connection.
    pub connection_diagnostics: &'static [DiagnosticSource],
}

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
pub(super) static RESTART: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static FRAMES: Channel<CriticalSectionRawMutex, (u32, HostFrame), 16> = Channel::new();
static LOGS: Channel<CriticalSectionRawMutex, heapless::String<192>, 32> = Channel::new();
struct Reply {
    generation: u32,
    session: Option<SessionId>,
    line: alloc::string::String,
    acknowledgement: Option<control::Ticket>,
}
static REPLIES: Channel<CriticalSectionRawMutex, Reply, 2> = Channel::new();
static REPLY_SENT: Signal<CriticalSectionRawMutex, (control::Ticket, bool)> = Signal::new();
static CONTROL: Mutex<CriticalSectionRawMutex, RefCell<control::State>> =
    Mutex::new(RefCell::new(control::State::new()));
static LOGGER: Logger = Logger;
static PAUSES: StaticCell<[Pause<CriticalSectionRawMutex>; 4]> = StaticCell::new();

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
pub(super) fn change(status: u32) {
    STATUS.store(status, Ordering::Release);
    let previous = GENERATION.fetch_add(1, Ordering::AcqRel);
    REPLIES.clear();
    CONTROL.lock(|state| state.borrow_mut().observe(previous.wrapping_add(1), false));
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

/// Initialize the composite device without launching its workers.
/// Launch all returned workers on the preparation core before using the lifecycle handle.
#[must_use = "launch the returned USB workers before using the lifecycle handle"]
pub fn prepare(
    peripheral: USB_FS<'static>,
    dp: GPIO20<'static>,
    dm: GPIO19<'static>,
    config: Config,
) -> (UsbLifecycle, UsbWorkers) {
    assert!(
        config.max_request_len <= REQUEST_CAPACITY,
        "USB request limit exceeds buffer capacity"
    );
    let [device, hid_pause, logs, commands] = PAUSES.init(core::array::from_fn(|_| Pause::new()));
    let (mut device, device_worker) = device.split();
    let (mut hid_pause, hid_pause_worker) = hid_pause.split();
    let (mut logs, logs_worker) = logs.split();
    let (mut commands, commands_worker) = commands.split();
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);
    let driver = Driver::new(
        Usb::new_fs(peripheral, dp, dm),
        OUT_BUFFER.init([0; 256]),
        DriverConfig::default(),
    );
    let mut builder = Builder::new(
        driver,
        config.device,
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
            report_descriptor: config.descriptor.as_bytes(),
            request_handler: Some(REQUESTS.init(Requests)),
            poll_ms: 8,
            max_packet_size: 64,
            hid_subclass: hid::HidSubclass::No,
            hid_boot_protocol: hid::HidBootProtocol::None,
        },
    );
    let cdc_workers = CdcWorkers::new(
        cdc,
        config.command_handler,
        config.max_request_len,
        config.connection_diagnostics,
        commands_worker,
        logs_worker,
    );
    let workers = UsbWorkers::new(
        DeviceWorker::new(builder.build(), device_worker),
        HidWorker::new(hid, hid_pause_worker),
        cdc_workers,
    );
    // All USB classes are initialized before any worker can launch.
    device.resume();
    hid_pause.resume();
    logs.resume();
    commands.resume();
    (
        UsbLifecycle {
            device,
            hid: hid_pause,
            logs,
            commands,
        },
        workers,
    )
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

pub struct UsbControl;

impl ReplyTransport for UsbControl {
    type Reply = alloc::string::String;

    fn is_session_active(&self, session: SessionId) -> bool {
        is_session_active(session)
    }

    async fn send_reply(&self, session: SessionId, reply: Self::Reply) -> bool {
        send_control_reply(session, reply).await
    }
}

pub fn is_session_active(session: SessionId) -> bool {
    let generation = GENERATION.load(Ordering::Acquire);
    STATUS.load(Ordering::Acquire) == 1
        && CONTROL.lock(|state| state.borrow().is_session_active(session, generation))
}

fn control_reply_live(ticket: control::Ticket) -> bool {
    is_session_active(ticket.session)
        && GENERATION.load(Ordering::Acquire) == ticket.generation
        && CONTROL.lock(|state| state.borrow().reply_live(ticket))
}

fn reply_session_live(generation: u32, session: Option<SessionId>) -> bool {
    GENERATION.load(Ordering::Acquire) == generation
        && CONTROL.lock(|state| state.borrow().reply_session_live(generation, session))
}

struct ReplyLease(control::Ticket);

impl Drop for ReplyLease {
    fn drop(&mut self) {
        CONTROL.lock(|state| state.borrow_mut().cancel_reply(self.0));
    }
}

/// Return success only after the CDC writer has sent the complete response.
pub async fn send_control_reply(session: SessionId, line: alloc::string::String) -> bool {
    if !is_session_active(session) {
        return false;
    }
    let generation = GENERATION.load(Ordering::Acquire);
    let Some(ticket) = CONTROL.lock(|state| state.borrow_mut().begin_reply(session, generation))
    else {
        return false;
    };
    // Cancellation also invalidates replies that are queued or between USB packets.
    let _lease = ReplyLease(ticket);
    matches!(
        with_timeout(Duration::from_secs(2), async {
            if !control_reply_live(ticket) {
                return false;
            }
            REPLIES
                .send(Reply {
                    generation,
                    session: Some(session),
                    line,
                    acknowledgement: Some(ticket),
                })
                .await;
            loop {
                let (completed, sent) = REPLY_SENT.wait().await;
                if completed == ticket {
                    return control_reply_live(ticket)
                        && CONTROL
                            .lock(|state| state.borrow().accepts_ack(ticket, completed, sent));
                }
            }
        })
        .await,
        Ok(true)
    )
}
