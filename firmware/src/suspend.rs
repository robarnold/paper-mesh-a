//! Explicit light sleep after service preparation and a cooperative core stop.
//!
//! # Design choices
//!
//! - Pause device services and park the display core before sleep so peripheral work
//!   cannot overlap the power transition.
//!
//! See [Manual light sleep](../../docs/boards/lilygo-t5-s3-pro-power.md#manual-light-sleep).

use core::{
    cell::RefCell,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embassy_time::{Duration, Instant, Timer, with_timeout};
use esp_hal::{
    gpio::{Event, Flex, InputConfig, Pull},
    peripherals::{GPIO38, LPWR, RTC_TIMER},
    rtc_cntl::{Rtc, WakeLock, sleep::LowPower},
};
use paper_mesh_platform::power::PowerSnapshot;
use paper_mesh_platform::suspend::{
    SuspendError, SuspendParticipant, SuspendReport, SuspendRequest,
};

use crate::bus::Bus;
pub mod cores;
mod expander;
mod park;
mod recovery;
pub mod trace;
mod wake;

const POWER_OFF_PREPARATION_TIMEOUT: Duration = Duration::from_secs(6);
const POWER_OFF_FAULT_RESET_MS: u64 = 1_000;

static TRANSITION_ACTIVE: AtomicBool = AtomicBool::new(false);
static WAKE_BITS: AtomicU32 = AtomicU32::new(0);
static PIN: Mutex<CriticalSectionRawMutex, RefCell<Option<Flex<'static>>>> =
    Mutex::new(RefCell::new(None));

pub struct Controller {
    low_power: LowPower<'static>,
    recovery: recovery::Recovery,
}

pub struct Prepared {
    pub controller: Controller,
    pub interrupt: paper_mesh_esp_adapters::interrupt::GpioInterruptHandler,
}

pub fn initialize(
    pin: GPIO38<'static>,
    lpwr: LPWR<'static>,
    rtc_timer: RTC_TIMER<'static>,
) -> Prepared {
    // Keep the application wake lock until preparation completes.
    WakeLock::acquire();
    let mut pin = Flex::new(pin);
    pin.set_input_enable(true);
    pin.apply_input_config(&InputConfig::default().with_pull(Pull::Up));
    PIN.lock(|slot| *slot.borrow_mut() = Some(pin));
    Prepared {
        controller: Controller {
            low_power: LowPower::new(lpwr),
            recovery: recovery::Recovery::new(Rtc::new(rtc_timer).rwdt),
        },
        interrupt: paper_mesh_esp_adapters::interrupt::GpioInterruptHandler::new(interrupt),
    }
}

/// Exclude new system operations while a power transition owns the hardware.
pub fn is_transition_active() -> bool {
    TRANSITION_ACTIVE.load(Ordering::Acquire)
}

fn set_transition_active(active: bool) {
    TRANSITION_ACTIVE.store(active, Ordering::Release);
}

fn interrupt() {
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

fn disarm() {
    if crate::buttons::take_boot_interrupt(true) {
        record(wake::BOOT);
    }
    if crate::features::touch_wake::finish() {
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

async fn prepare(
    bus: &Bus,
    touch_wake: bool,
    participant: &mut impl SuspendParticipant,
    deadline_ms: u64,
) -> Result<(PowerSnapshot, [u8; 2]), SuspendError> {
    participant
        .will_suspend(deadline_ms)
        .await
        .map_err(|_| SuspendError::Preparation)?;
    {
        let mut bus = bus.lock().await;
        expander::quiet_unused(&mut *bus).map_err(|_| SuspendError::Preparation)?;
    }
    // TPS65185 discharge continues after PWRGOOD falls and WAKEUP goes low.
    // Its later status transition must finish before GPIO38 becomes a wake input.
    Timer::after_millis(500).await;
    trace::mark(trace::Stage::ReadingBefore);
    let before = crate::power::observation::read_now(bus).await;
    let mut bus = bus.lock().await;
    // Clear both expander interrupt comparisons after the display and GNSS stop.
    let ports = expander::inputs(&mut *bus).map_err(|_| SuspendError::Preparation)?;
    if ports[1] & 4 == 0 {
        return Err(SuspendError::WakeActive);
    }
    crate::buttons::arm_boot_wake()?;
    if touch_wake {
        crate::features::touch_wake::arm()?;
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

async fn restore_participant(
    participant: &mut impl SuspendParticipant,
    slept: bool,
    deadline_ms: u64,
) -> Result<(), ()> {
    if slept {
        participant.did_resume(deadline_ms).await.map_err(|_| ())
    } else {
        participant
            .did_cancel_suspend(deadline_ms)
            .await
            .map_err(|_| ())
    }
}

pub(crate) async fn reset_after_power_off_fault(reason: &str) -> ! {
    log::error!("power off: {}; waiting for reset recovery", reason);
    set_transition_active(true);
    let mut armed =
        crate::recovery::arm(crate::recovery::Owner::PowerOff, POWER_OFF_FAULT_RESET_MS);
    // Keep workers stopped. Normal startup restores hardware under the boot watchdog.
    loop {
        Timer::after_secs(1).await;
        // Preserve another operation's deadline, but claim recovery if that operation finishes.
        if !armed {
            armed =
                crate::recovery::arm(crate::recovery::Owner::PowerOff, POWER_OFF_FAULT_RESET_MS);
        }
    }
}

impl Controller {
    pub async fn power_off(
        &mut self,
        bus: &Bus,
        participant: &mut impl paper_mesh_platform::power_off::PowerOffParticipant,
    ) -> Result<(), paper_mesh_platform::power_off::PowerOffError> {
        use crate::recovery::{self, Owner};
        use paper_mesh_platform::power_off::PowerOffError;
        if is_transition_active()
            || crate::discovery::is_active()
            || crate::display_worker::is_refreshing()
            || !recovery::arm(Owner::PowerOff, 15_000)
        {
            return Err(PowerOffError::Busy);
        }
        set_transition_active(true);
        let mut controls_attempted = false;
        let mut controls_confirmed = false;
        let preparation_deadline = (Instant::now() + POWER_OFF_PREPARATION_TIMEOUT).as_millis();
        let prepared = with_timeout(POWER_OFF_PREPARATION_TIMEOUT, async {
            participant
                .will_power_off(preparation_deadline)
                .await
                .map_err(|_| PowerOffError::Preparation)?;
            // Also clear retained controls when display startup or recovery failed.
            controls_attempted = true;
            crate::display::establish_controls_off(bus).map_err(|_| PowerOffError::Preparation)?;
            controls_confirmed = true;
            crate::power::check_power_off(bus).await
        })
        .await;
        if controls_attempted && !controls_confirmed {
            crate::display_recovery::latch_fault(crate::display_recovery::Stage::ShuttingDown);
            reset_after_power_off_fault("display shutdown unconfirmed").await;
        }
        match prepared {
            Ok(Ok(())) => {
                let cut = {
                    let mut bus = bus.lock().await;
                    crate::power::ship::cut(&mut *bus)
                };
                if matches!(cut, Err(crate::power::ship::Error::ExternalPower)) {
                    reset_after_power_off_fault("external power appeared after preparation").await;
                }
                // The write may take effect despite a missing ACK. Never retry or resume here.
                Timer::after_millis(1_000).await;
                reset_after_power_off_fault("power cut did not stop execution").await;
            }
            Ok(Err(_)) => reset_after_power_off_fault("preparation failed").await,
            Err(_) => reset_after_power_off_fault("preparation timed out").await,
        }
    }

    // Separate transition futures from restoration to bound coordinator stack use.
    async fn suspend_once(
        &mut self,
        bus: &Bus,
        request: SuspendRequest,
        participant: &mut impl SuspendParticipant,
    ) -> Result<SuspendReport, SuspendError> {
        let preparation_deadline = Instant::now() + Duration::from_secs(5);
        match with_timeout(
            Duration::from_secs(5),
            prepare(
                bus,
                request.touch_wake,
                participant,
                preparation_deadline.as_millis(),
            ),
        )
        .await
        {
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
                let after = crate::power::observation::read_now(bus).await;
                result.map(|(elapsed_ms, entered_sleep)| SuspendReport {
                    entered_sleep,
                    elapsed_ms,
                    wake_reasons,
                    before,
                    after,
                })
            }
            Ok(Err(error)) => Err(error),
            Err(_) => Err(SuspendError::Preparation),
        }
    }

    pub async fn run(
        &mut self,
        bus: &Bus,
        request: SuspendRequest,
        participant: &mut impl SuspendParticipant,
    ) -> Result<SuspendReport, SuspendError> {
        if is_transition_active() || crate::discovery::is_active() {
            return Err(SuspendError::Busy);
        }
        if request.duration_ms > 3_600_000 {
            return Err(SuspendError::Unsupported);
        }
        if !self.recovery.arm(15_000) {
            return Err(SuspendError::Busy);
        }
        WAKE_BITS.store(0, Ordering::Release);
        trace::mark(trace::Stage::Preparing);
        set_transition_active(true);
        let result = self.suspend_once(bus, request, participant).await;
        let entered_sleep = result.as_ref().is_ok_and(|report| report.entered_sleep);
        disarm();
        trace::mark(trace::Stage::ResumingServices);
        let restoration_deadline = (Instant::now() + Duration::from_secs(1)).as_millis();
        let restored = with_timeout(
            Duration::from_secs(1),
            restore_participant(participant, entered_sleep, restoration_deadline),
        )
        .await;
        if !matches!(restored, Ok(Ok(()))) {
            log::error!("suspend: participant restoration failed or timed out");
            // Keep workers stopped and the recovery watchdog armed after failed restoration.
            loop {
                Timer::after_secs(1).await;
            }
        }
        set_transition_active(false);
        trace::mark(if result.is_ok() {
            trace::Stage::Complete
        } else {
            trace::Stage::Failed
        });
        self.recovery.disable();
        result
    }
}
