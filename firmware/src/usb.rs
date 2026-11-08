//! Composite USB transport. HID input and diagnostic CDC share the internal PHY.
//!
//! # Design choices
//!
//! - Expose touchpad reports and diagnostic logs through one composite device because
//!   both functions share the internal USB PHY.
//!
//! See [LILYGO USB touchpad and diagnostics](../../docs/boards/lilygo-t5-s3-pro-usb.md).

mod control;
mod lifecycle;
pub use lifecycle::UsbLifecycle;
use lifecycle::Worker;
use paper_mesh_platform::worker::Pause;

use paper_mesh_platform::transport::{ReplyTransport, SessionId, framing::FrameError};

/// Runtime-selected dispatch for a complete request or framing error in its session.
pub type CommandHandler = fn(Result<&[u8], FrameError>, SessionId) -> Option<alloc::string::String>;

/// Supply one connection diagnostic without waiting or accessing USB.
pub type DiagnosticSource = fn() -> heapless::String<192>;

use core::{
    cell::RefCell,
    fmt::Write,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    channel::Channel,
    signal::Signal,
};
use embassy_time::{Duration, Instant, Timer, with_timeout};
use embassy_usb::{
    Builder, Handler, UsbDevice,
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
use paper_mesh_hid_touchpad::{Descriptor, Encoder, REPORT_LEN};
use paper_mesh_platform::touchpad::{HostFrame, HostState, TouchpadHost};
use static_cell::StaticCell;

/// Runtime-selected device identity, touchpad geometry, and command dispatch.
pub struct Config {
    pub device: embassy_usb::Config<'static>,
    /// Must describe the logical coordinates submitted through TouchpadHost.
    pub descriptor: &'static Descriptor,
    pub command_handler: CommandHandler,
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
static RESTART: Signal<CriticalSectionRawMutex, ()> = Signal::new();
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
fn change(status: u32) {
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

pub fn start(
    spawner: Spawner,
    peripheral: USB_FS<'static>,
    dp: GPIO20<'static>,
    dm: GPIO19<'static>,
    config: Config,
) -> UsbLifecycle {
    let [device, hid_pause, logs, commands] = PAUSES.init(core::array::from_fn(|_| Pause::new()));
    let (device, device_worker) = device.split();
    let (hid_pause, hid_pause_worker) = hid_pause.split();
    let (logs, logs_worker) = logs.split();
    let (commands, commands_worker) = commands.split();
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
    let (sender, receiver, control) = cdc.split_with_control();
    spawner
        .spawn(command_worker(receiver, control, config.command_handler, commands_worker).unwrap());
    spawner.spawn(bus_worker(builder.build(), device_worker).unwrap());
    spawner.spawn(hid_worker(hid, hid_pause_worker).unwrap());
    spawner.spawn(log_worker(sender, logs_worker, config.connection_diagnostics).unwrap());
    UsbLifecycle {
        device,
        hid: hid_pause,
        logs,
        commands,
    }
}

#[embassy_executor::task]
async fn bus_worker(mut device: UsbDevice<'static, Driver<'static>>, mut pause: Worker) {
    loop {
        select(device.run(), select(RESTART.wait(), pause.requested())).await;
        // The resolved Synopsys driver's disable() does not disconnect the PHY.
        // Keep clocks running and detach before resetting the USB stack state.
        USB_FS::regs().pcgcctl().modify(|_, w| {
            w.stoppclk().clear_bit();
            w.gatehclk().clear_bit()
        });
        USB_FS::regs().dctl().modify(|_, w| w.sftdiscon().set_bit());
        // Embassy requires disable after cancellation of run().
        device.disable().await;
        loop {
            pause.checkpoint().await;
            if matches!(
                select(pause.requested(), Timer::after_millis(250)).await,
                Either::Second(())
            ) {
                break;
            }
        }
        // A restart observed while detached is already satisfied by this reconnect.
        RESTART.reset();
        USB_FS::regs()
            .dctl()
            .modify(|_, w| w.sftdiscon().clear_bit());
        log::info!("USB: reconnect");
    }
}

#[embassy_executor::task]
async fn hid_worker(
    mut writer: hid::HidWriter<'static, Driver<'static>, REPORT_LEN>,
    mut pause: Worker,
) {
    loop {
        pause.checkpoint().await;
        select(pause.requested(), write_hid(&mut writer)).await;
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
async fn log_worker(
    mut sender: cdc_acm::Sender<'static, Driver<'static>>,
    mut pause: Worker,
    diagnostics: &'static [DiagnosticSource],
) {
    loop {
        pause.checkpoint().await;
        select(pause.requested(), write_logs(&mut sender, diagnostics)).await;
    }
}

async fn write_logs(
    sender: &mut cdc_acm::Sender<'static, Driver<'static>>,
    diagnostics: &'static [DiagnosticSource],
) {
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
            // End any partial line that the previous connection received.
            if !send_log_line(sender, "\n", generation).await {
                continue;
            }
            if send_connection_info(sender, generation, diagnostics).await {
                connection = Some(generation);
            }
            continue;
        }
        let (line, session, acknowledgement) = match select(
            REPLIES.receive(),
            select(LOGS.receive(), Timer::after_millis(250)),
        )
        .await
        {
            Either::First(reply) => {
                if reply.generation != generation
                    || !reply_session_live(generation, reply.session)
                    || reply
                        .acknowledgement
                        .is_some_and(|ticket| !control_reply_live(ticket))
                {
                    if let Some(ticket) = reply.acknowledgement {
                        REPLY_SENT.signal((ticket, false));
                    }
                    continue;
                }
                (reply.line, reply.session, reply.acknowledgement)
            }
            Either::Second(Either::First(line)) => {
                (alloc::string::String::from(line.as_str()), None, None)
            }
            _ => continue,
        };
        let sent = send_line(sender, &line, generation, session, acknowledgement).await;
        if let Some(ticket) = acknowledgement {
            REPLY_SENT.signal((ticket, sent && control_reply_live(ticket)));
        }
        if !sent {
            connection = None;
        }
    }
}

fn send_connection_info<'a>(
    sender: &'a mut cdc_acm::Sender<'static, Driver<'static>>,
    generation: u32,
    diagnostics: &'static [DiagnosticSource],
) -> core::pin::Pin<alloc::boxed::Box<dyn core::future::Future<Output = bool> + 'a>> {
    alloc::boxed::Box::pin(async move {
        for source in diagnostics {
            let line = source();
            if !send_log_line(sender, &line, generation).await {
                return false;
            }
        }
        true
    })
}

#[embassy_executor::task]
async fn command_worker(
    mut receiver: cdc_acm::Receiver<'static, Driver<'static>>,
    control: cdc_acm::ControlChanged<'static>,
    handler: CommandHandler,
    mut pause: Worker,
) {
    loop {
        pause.checkpoint().await;
        select(
            pause.requested(),
            read_commands(&mut receiver, &control, handler),
        )
        .await;
    }
}

fn read_commands<'a>(
    receiver: &'a mut cdc_acm::Receiver<'static, Driver<'static>>,
    control: &'a cdc_acm::ControlChanged<'static>,
    handler: CommandHandler,
) -> core::pin::Pin<alloc::boxed::Box<dyn core::future::Future<Output = ()> + 'a>> {
    // Keep command framing outside the task's cancellation wrapper to bound stack use.
    alloc::boxed::Box::pin(read_commands_inner(receiver, control, handler))
}

async fn read_commands_inner(
    receiver: &mut cdc_acm::Receiver<'static, Driver<'static>>,
    control: &cdc_acm::ControlChanged<'static>,
    handler: CommandHandler,
) {
    use crate::usb_protocol as protocol;
    use paper_mesh_platform::transport::framing::LineFramer;
    let mut frame = LineFramer::<{ protocol::MAX_REQUEST }>::default();
    let mut observed_generation = None;
    let mut control_session = None;
    let mut packet = [0; 64];
    loop {
        let generation = GENERATION.load(Ordering::Acquire);
        let connected = STATUS.load(Ordering::Acquire) == 1 && receiver.dtr();
        CONTROL.lock(|state| state.borrow_mut().observe(generation, connected));
        let observed_session = CONTROL.lock(|state| state.borrow().session(generation));
        if control_session != observed_session {
            frame.clear();
            control_session = observed_session;
        }
        if observed_generation != Some(generation) || !connected {
            if let Some(previous) = observed_generation.take() {
                command_disconnected(previous);
                REPLIES.clear();
            }
            frame.clear();
            if !connected {
                Timer::after_millis(50).await;
                continue;
            }
            observed_generation = Some(generation);
        }
        let length = match with_timeout(
            Duration::from_millis(100),
            select(control.control_changed(), receiver.read_packet(&mut packet)),
        )
        .await
        {
            Ok(Either::First(())) => {
                // Notifications can combine a close and reopen. Invalidate the old session
                // even when DTR is already high when the worker receives the event.
                CONTROL.lock(|state| state.borrow_mut().disconnect(generation));
                if !control.dtr() {
                    command_disconnected(generation);
                }
                frame.clear();
                continue;
            }
            Ok(Either::Second(Ok(length))) => length,
            Ok(Either::Second(Err(_))) => {
                command_disconnected(generation);
                frame.clear();
                Timer::after_millis(50).await;
                continue;
            }
            Err(_) => continue,
        };
        if GENERATION.load(Ordering::Acquire) != generation || !receiver.dtr() {
            if !receiver.dtr() {
                command_disconnected(generation);
            }
            frame.clear();
            continue;
        }
        let Some(packet_session) = control_session else {
            frame.clear();
            continue;
        };
        for byte in &packet[..length] {
            if !is_session_active(packet_session) {
                frame.clear();
                break;
            }
            let Some(message) = frame.push(*byte) else {
                continue;
            };
            let reply = match message {
                Ok(bytes) => handler(Ok(&bytes), packet_session),
                Err(error) => handler(Err(error), packet_session),
            };
            if let Some(reply) = reply
                && with_timeout(
                    Duration::from_millis(250),
                    REPLIES.send(Reply {
                        generation,
                        // Errors belong to the same CDC session as their requests.
                        session: Some(packet_session),
                        line: reply,
                        acknowledgement: None,
                    }),
                )
                .await
                .is_err()
            {
                command_disconnected(generation);
                frame.clear();
                break;
            }
        }
    }
}

fn command_disconnected(generation: u32) {
    CONTROL.lock(|state| state.borrow_mut().disconnect(generation));
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

fn log_connection(sender: &cdc_acm::Sender<'static, Driver<'static>>, generation: u32) -> bool {
    let connected = STATUS.load(Ordering::Acquire) == 1
        && GENERATION.load(Ordering::Acquire) == generation
        && sender.dtr();
    if !connected {
        CONTROL.lock(|state| state.borrow_mut().disconnect(generation));
    }
    connected
}

async fn send_log_line(
    sender: &mut cdc_acm::Sender<'static, Driver<'static>>,
    line: &str,
    generation: u32,
) -> bool {
    send_line(sender, line, generation, None, None).await
}

async fn send_line(
    sender: &mut cdc_acm::Sender<'static, Driver<'static>>,
    line: &str,
    generation: u32,
    session: Option<SessionId>,
    acknowledgement: Option<control::Ticket>,
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
            || !reply_session_live(generation, session)
            || acknowledgement.is_some_and(|ticket| !control_reply_live(ticket))
            || !matches!(
                with_timeout(Duration::from_millis(250), sender.write_packet(packet)).await,
                Ok(Ok(()))
            )
        {
            return false;
        }
    }
    log_connection(sender, generation)
        && reply_session_live(generation, session)
        && acknowledgement.is_none_or(control_reply_live)
}
