//! Explicit light sleep after service preparation and a cooperative core stop.

use core::{
    cell::RefCell,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
use embassy_futures::select::select;
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    signal::Signal,
};
use embassy_time::{Duration, Instant, Timer, with_timeout};
use esp_hal::{
    gpio::{Event, Flex, InputConfig, Pull},
    peripherals::{GPIO38, LPWR, RTC_TIMER},
    rtc_cntl::{Rtc, WakeLock, sleep::LowPower},
};
use paper_mesh_platform::power::PowerSnapshot;
use paper_mesh_platform::suspend::{SuspendError, SuspendReport, SuspendRequest};

use crate::bus::Bus;
pub mod cores;
mod expander;
mod park;
mod recovery;
pub mod trace;
mod wake;

#[derive(Clone, Copy)]
pub enum Service {
    Buttons,
    Gnss,
    Power,
    Rtc,
    Touch,
    Usb,
    Hid,
    Logs,
}

static REQUESTED: AtomicBool = AtomicBool::new(false);
static REGISTERED: AtomicU32 = AtomicU32::new(0);
static PAUSED: AtomicU32 = AtomicU32::new(0);
static FAILED: AtomicBool = AtomicBool::new(false);
static SIGNALS: [Signal<CriticalSectionRawMutex, ()>; 8] = [const { Signal::new() }; 8];
static WAKE_BITS: AtomicU32 = AtomicU32::new(0);
static PIN: Mutex<CriticalSectionRawMutex, RefCell<Option<Flex<'static>>>> =
    Mutex::new(RefCell::new(None));

pub struct Controller {
    low_power: LowPower<'static>,
    recovery: recovery::Recovery,
}

pub fn initialize(
    pin: GPIO38<'static>,
    lpwr: LPWR<'static>,
    rtc_timer: RTC_TIMER<'static>,
) -> Controller {
    // Keep the application wake lock until preparation completes.
    WakeLock::acquire();
    let mut pin = Flex::new(pin);
    pin.set_input_enable(true);
    pin.apply_input_config(&InputConfig::default().with_pull(Pull::Up));
    PIN.lock(|slot| *slot.borrow_mut() = Some(pin));
    Controller {
        low_power: LowPower::new(lpwr),
        recovery: recovery::Recovery::new(Rtc::new(rtc_timer).rwdt),
    }
}

pub fn register(service: Service) {
    REGISTERED.fetch_or(1 << service as u32, Ordering::AcqRel);
}
pub fn requested() -> bool {
    REQUESTED.load(Ordering::Acquire)
}
pub fn fail() {
    FAILED.store(true, Ordering::Release);
}

pub async fn request(service: Service) {
    while !requested() {
        SIGNALS[service as usize].wait().await;
    }
}

pub async fn checkpoint(service: Service) {
    if !requested() {
        return;
    }
    let bit = 1 << service as u32;
    PAUSED.fetch_or(bit, Ordering::AcqRel);
    while requested() {
        SIGNALS[service as usize].wait().await;
    }
    PAUSED.fetch_and(!bit, Ordering::AcqRel);
}

pub async fn delay(service: Service, milliseconds: u64) {
    select(Timer::after_millis(milliseconds), request(service)).await;
}

fn notify(paused: bool) {
    REQUESTED.store(paused, Ordering::Release);
    for signal in &SIGNALS {
        signal.signal(());
    }
}

pub fn interrupt() {
    if crate::buttons::take_boot_interrupt(false) {
        record(wake::BOOT);
    }
    PIN.lock(|slot| {
        if let Some(pin) = slot.borrow_mut().as_mut()
            && pin.is_interrupt_set()
        {
            pin.unlisten();
            pin.clear_interrupt();
            record(wake::EXPANDER);
        }
    });
}

fn record(bits: u32) {
    WAKE_BITS.fetch_or(bits, Ordering::AcqRel);
}

pub fn touch_interrupt() {
    record(wake::TOUCH);
}

pub fn touch_woke() -> bool {
    WAKE_BITS.load(Ordering::Acquire) & wake::TOUCH != 0
}

fn disarm() {
    if crate::buttons::take_boot_interrupt(true) {
        record(wake::BOOT);
    }
    if crate::touch::disarm_wake() {
        record(wake::TOUCH);
    }
    PIN.lock(|slot| {
        if let Some(pin) = slot.borrow_mut().as_mut() {
            if pin.is_interrupt_set() {
                record(wake::EXPANDER);
            }
            pin.unlisten();
            pin.clear_interrupt();
        }
    });
}

async fn prepare(bus: &Bus, touch_wake: bool) -> Result<(PowerSnapshot, [u8; 2]), SuspendError> {
    while PAUSED.load(Ordering::Acquire) != REGISTERED.load(Ordering::Acquire) {
        Timer::after_millis(1).await;
    }
    if FAILED.load(Ordering::Acquire) {
        return Err(SuspendError::Preparation);
    }
    {
        let mut bus = bus.lock().await;
        expander::quiet_unused(&mut *bus).map_err(|_| SuspendError::Preparation)?;
    }
    // TPS65185 discharge continues after PWRGOOD falls and WAKEUP goes low.
    // Its later status transition must finish before GPIO38 becomes a wake input.
    Timer::after_millis(500).await;
    trace::mark(trace::Stage::ReadingBefore);
    let before = crate::power::read_now(bus).await;
    let mut bus = bus.lock().await;
    // Clear both expander interrupt comparisons after the display and GNSS stop.
    let ports = expander::inputs(&mut *bus).map_err(|_| SuspendError::Preparation)?;
    if ports[1] & 4 == 0 {
        return Err(SuspendError::WakeActive);
    }
    crate::buttons::arm_boot_wake()?;
    if touch_wake {
        crate::touch::arm_wake()?;
    }
    PIN.lock(|slot| {
        let mut slot = slot.borrow_mut();
        let pin = slot.as_mut().ok_or(SuspendError::Preparation)?;
        if pin.is_low() {
            return Err(SuspendError::WakeActive);
        }
        pin.clear_interrupt();
        pin.listen(Event::LowLevel);
        Ok(())
    })?;
    Ok((before, ports))
}

impl Controller {
    pub async fn run(
        &mut self,
        bus: &Bus,
        request: SuspendRequest,
    ) -> Result<SuspendReport, SuspendError> {
        if requested() || PAUSED.load(Ordering::Acquire) != 0 || crate::discovery::is_active() {
            return Err(SuspendError::Busy);
        }
        if request.duration_ms > 3_600_000 {
            return Err(SuspendError::Unsupported);
        }
        if !self.recovery.arm(15_000) {
            return Err(SuspendError::Busy);
        }
        FAILED.store(false, Ordering::Release);
        WAKE_BITS.store(0, Ordering::Release);
        trace::mark(trace::Stage::Preparing);
        notify(true);
        let result =
            match with_timeout(Duration::from_secs(5), prepare(bus, request.touch_wake)).await {
                Ok(Ok((before, ports_before))) => {
                    trace::mark(trace::Stage::ParkingCore);
                    let parked = with_timeout(Duration::from_secs(1), cores::request_park()).await;
                    let started = Instant::now();
                    let deadline = started + Duration::from_millis(request.duration_ms);
                    let result = if !parked.is_ok_and(|result| result.is_ok()) {
                        Err(SuspendError::Preparation)
                    } else {
                        if self.recovery.arm(request.duration_ms + 15_000) {
                            cores::sleep(&mut self.low_power, deadline)
                                .map(|entered| (started.elapsed().as_millis(), entered))
                        } else {
                            Err(SuspendError::Preparation)
                        }
                    };
                    self.recovery.arm(15_000);
                    cores::release();
                    // Capture all pending inputs before bus reads or service restoration.
                    disarm();
                    let captured = WAKE_BITS.load(Ordering::Acquire);
                    let timer = Instant::now() >= deadline;
                    let ports_after = expander::inputs(&mut *bus.lock().await).ok();
                    let wake_reasons =
                        wake::reasons(captured, ports_after.map(|ports| ports[1]), timer);
                    trace::record_wake(captured, ports_before, ports_after, result.ok(), timer);
                    trace::mark(trace::Stage::ReadingAfter);
                    let after = crate::power::read_now(bus).await;
                    result.map(|(elapsed_ms, _)| SuspendReport {
                        elapsed_ms,
                        wake_reasons,
                        before,
                        after,
                    })
                }
                Ok(Err(error)) => Err(error),
                Err(_) => Err(SuspendError::Preparation),
            };
        disarm();
        trace::mark(trace::Stage::ResumingServices);
        notify(false);
        trace::mark(if result.is_ok() {
            trace::Stage::Complete
        } else {
            trace::Stage::Failed
        });
        self.recovery.disable();
        result
    }
}
