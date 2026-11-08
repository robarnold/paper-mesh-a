//! Prepared USB workers whose placement belongs to the runtime.
//!
//! # Design choices
//!
//! - Retain concrete resource types so task ownership and thread restrictions remain visible to Rust.
//! - Create each Embassy spawn token only when its selected spawner can consume it immediately.
//!
//! See [worker preparation and placement](../../../../../docs/adapters/usb.md#worker-preparation-and-placement).

use embassy_executor::{SpawnError, Spawner};
use embassy_usb::{
    UsbDevice,
    class::{cdc_acm, hid},
};
use esp_hal::usb::otg::embassy_usb_device::Driver;
use paper_mesh_hid_touchpad::REPORT_LEN;

use super::{
    ACTIVE, CONTROL, CommandHandler, DiagnosticSource, FRAMES, FirmwareTouchpad, GENERATION, LOGS,
    REPLIES, REPLY_SENT, REQUEST_CAPACITY, RESTART, Reply, STATUS, control, control_reply_live,
    is_session_active, reply_session_live,
};
use crate::usb::lifecycle::Worker;
use core::sync::atomic::Ordering;
use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Instant, Timer, with_timeout};
use esp_hal::peripherals::USB_FS;
use paper_mesh_hid_touchpad::Encoder;
use paper_mesh_platform::{
    touchpad::{HostFrame, TouchpadHost},
    transport::{SessionId, framing::LineFramer},
    worker::WorkerSet,
};

type RequestFramer = LineFramer<REQUEST_CAPACITY>;

/// All workers required by one composite USB device. Launch every member before using its lifecycle handle.
#[must_use = "prepared USB workers must be launched before using the adapter"]
pub struct UsbWorkers {
    device: DeviceWorker,
    commands: CommandWorker,
    hid: HidWorker,
    logs: LogWorker,
}

impl UsbWorkers {
    pub(super) fn new(device: DeviceWorker, hid: HidWorker, cdc: CdcWorkers) -> Self {
        Self {
            device,
            hid,
            commands: cdc.commands,
            logs: cdc.logs,
        }
    }
}

/// Prepare both CDC workers together because they share one class and connection state.
#[must_use = "both CDC workers must be included in the USB worker set"]
pub(super) struct CdcWorkers {
    commands: CommandWorker,
    logs: LogWorker,
}

impl CdcWorkers {
    pub(super) fn new(
        cdc: cdc_acm::CdcAcmClass<'static, Driver<'static>>,
        handler: CommandHandler,
        max_request_len: usize,
        diagnostics: &'static [DiagnosticSource],
        commands_pause: Worker,
        logs_pause: Worker,
    ) -> Self {
        let (sender, receiver, control) = cdc.split_with_control();
        Self {
            commands: CommandWorker {
                receiver,
                control,
                handler,
                pause: commands_pause,
                max_request_len,
            },
            logs: LogWorker {
                sender,
                pause: logs_pause,
                diagnostics,
            },
        }
    }
}

#[must_use = "the prepared device worker must be launched"]
pub(super) struct DeviceWorker {
    device: UsbDevice<'static, Driver<'static>>,
    pause: Worker,
}

impl DeviceWorker {
    pub(super) fn new(device: UsbDevice<'static, Driver<'static>>, pause: Worker) -> Self {
        Self { device, pause }
    }

    fn spawn(self, spawner: Spawner) -> Result<(), SpawnError> {
        spawner.spawn(bus_worker(self.device, self.pause)?);
        Ok(())
    }
}

#[must_use = "the prepared command worker must be launched"]
struct CommandWorker {
    receiver: cdc_acm::Receiver<'static, Driver<'static>>,
    control: cdc_acm::ControlChanged<'static>,
    handler: CommandHandler,
    pause: Worker,
    max_request_len: usize,
}

impl CommandWorker {
    fn spawn(self, spawner: Spawner) -> Result<(), SpawnError> {
        spawner.spawn(command_worker(
            self.receiver,
            self.control,
            self.handler,
            self.pause,
            self.max_request_len,
        )?);
        Ok(())
    }
}

#[must_use = "the prepared HID worker must be launched"]
pub(super) struct HidWorker {
    writer: hid::HidWriter<'static, Driver<'static>, REPORT_LEN>,
    pause: Worker,
}

impl HidWorker {
    pub(super) fn new(
        writer: hid::HidWriter<'static, Driver<'static>, REPORT_LEN>,
        pause: Worker,
    ) -> Self {
        Self { writer, pause }
    }

    fn spawn(self, spawner: Spawner) -> Result<(), SpawnError> {
        spawner.spawn(hid_worker(self.writer, self.pause)?);
        Ok(())
    }
}

#[must_use = "the prepared log worker must be launched"]
struct LogWorker {
    sender: cdc_acm::Sender<'static, Driver<'static>>,
    pause: Worker,
    diagnostics: &'static [DiagnosticSource],
}

impl LogWorker {
    fn spawn(self, spawner: Spawner) -> Result<(), SpawnError> {
        spawner.spawn(log_worker(self.sender, self.pause, self.diagnostics)?);
        Ok(())
    }
}

impl WorkerSet for UsbWorkers {
    fn spawn(self, spawner: Spawner) -> Result<(), SpawnError> {
        // Preserve the composite device's existing launch order.
        self.commands.spawn(spawner)?;
        self.device.spawn(spawner)?;
        self.hid.spawn(spawner)?;
        self.logs.spawn(spawner)?;
        Ok(())
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
    max_request_len: usize,
) {
    // Construct storage in the task to keep it off the startup stack.
    let mut frame = RequestFramer::with_limit(max_request_len).expect("validated at USB startup");
    loop {
        pause.checkpoint().await;
        // Canceled reads must not retain partial requests after lifecycle restoration.
        frame.clear();
        // Boxing keeps the cancellation wrapper within the firmware stack limit.
        select(
            pause.requested(),
            alloc::boxed::Box::pin(read_commands(&mut receiver, &control, handler, &mut frame)),
        )
        .await;
    }
}

async fn read_commands(
    receiver: &mut cdc_acm::Receiver<'static, Driver<'static>>,
    control: &cdc_acm::ControlChanged<'static>,
    handler: CommandHandler,
    frame: &mut RequestFramer,
) {
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
