//! Retained suspend progress for diagnosis after an interrupted experiment.

use core::{
    cell::RefCell,
    fmt::Write,
    sync::atomic::{AtomicU32, Ordering},
};
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};

const MAGIC: u32 = 0x5355_5300;

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum Stage {
    Preparing = 1,
    ReadingBefore,
    ParkingCore,
    CoreParked,
    EnteringSleep,
    SleepReturned,
    AlarmRestored,
    ReadingAfter,
    ResumingServices,
    Complete,
    Failed,
}

#[esp_hal::ram(unstable(rtc_slow, persistent))]
static mut RETAINED: u32 = 0;
static PREVIOUS: AtomicU32 = AtomicU32::new(0);
static LAST_WAKE: Mutex<CriticalSectionRawMutex, RefCell<heapless::String<192>>> =
    Mutex::new(RefCell::new(heapless::String::new()));

pub fn record_wake(
    captured: u32,
    before: [u8; 2],
    after: Option<[u8; 2]>,
    sleep: Option<(u64, bool)>,
    timer: bool,
) {
    let mut line = heapless::String::new();
    let _ = writeln!(
        line,
        "INFO suspend: inputs=0x{:x}; ports={:02x?}->{:02x?}; elapsed_ms={:?}; HAL_called={}; timer={}",
        captured,
        before,
        after,
        sleep.map(|(elapsed, _)| elapsed),
        sleep.is_some_and(|(_, entered)| entered),
        timer
    );
    log::info!("{}", line.trim_start_matches("INFO ").trim_end());
    LAST_WAKE.lock(|last| *last.borrow_mut() = line);
}

pub fn wake_line() -> heapless::String<192> {
    LAST_WAKE.lock(|last| last.borrow().clone())
}

fn retained() -> &'static AtomicU32 {
    // SAFETY: The retained word is aligned and has only atomic accesses during execution.
    unsafe { AtomicU32::from_ptr(&raw mut RETAINED) }
}

pub fn boot() {
    let previous = retained().swap(0, Ordering::AcqRel);
    if previous & 0xffff_ff00 == MAGIC {
        PREVIOUS.store(previous, Ordering::Release);
        log::info!("suspend: previous boot stopped at {}", label(previous));
    }
}

#[inline]
pub fn mark(stage: Stage) {
    retained().store(MAGIC | stage as u32, Ordering::Release);
}

fn label(value: u32) -> &'static str {
    match value & 0xff {
        1 => "prepare services",
        2 => "read before",
        3 => "request core stop",
        4 => "core stopped",
        5 => "enter HAL sleep",
        6 => "HAL returned",
        7 => "alarm restored",
        8 => "read after",
        9 => "resume services",
        10 => "complete",
        11 => "failed",
        _ => "none",
    }
}

pub fn connection_line() -> heapless::String<192> {
    let mut line = heapless::String::new();
    let _ = writeln!(
        line,
        "INFO suspend: previous={}; current={}; paused=0x{:x}; registered=0x{:x}; reset={:?}",
        label(PREVIOUS.load(Ordering::Acquire)),
        label(retained().load(Ordering::Acquire)),
        super::PAUSED.load(Ordering::Acquire),
        super::REGISTERED.load(Ordering::Acquire),
        esp_hal::rtc_cntl::reset_reason(esp_hal::system::Cpu::ProCpu)
    );
    line
}
