//! Bound a display operation and retain its last checkpoint across CPU resets.

mod state;
pub use state::Stage;

use crate::recovery::{self, Owner};
use core::{
    fmt::Write,
    sync::atomic::{AtomicU32, Ordering},
};
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use state::{Record, Status};

#[esp_hal::ram(unstable(rtc_slow, persistent))]
static mut RETAINED: u32 = 0;
#[esp_hal::ram(unstable(rtc_slow, persistent))]
static mut PANIC_RETAINED: u32 = 0;
static CURRENT: AtomicU32 = AtomicU32::new(0);
static PANIC_CURRENT: AtomicU32 = AtomicU32::new(0);
static PREVIOUS: AtomicU32 = AtomicU32::new(0);
static UPDATE: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());

fn retained() -> &'static AtomicU32 {
    // SAFETY: This aligned RTC word has only atomic loads and stores.
    unsafe { AtomicU32::from_ptr(&raw mut RETAINED) }
}

fn panic_retained() -> &'static AtomicU32 {
    // SAFETY: This aligned RTC word has only atomic loads and stores.
    unsafe { AtomicU32::from_ptr(&raw mut PANIC_RETAINED) }
}

fn current_word() -> u32 {
    state::select(
        CURRENT.load(Ordering::Acquire),
        PANIC_CURRENT.load(Ordering::Acquire),
    )
    .map_or(0, Record::encode)
}

fn publish(word: u32) {
    CURRENT.store(word, Ordering::Release);
    // RTC memory needs only aligned word access. The mutex resides in DRAM.
    retained().store(word, Ordering::Release);
}

fn update(change: impl FnOnce(u32) -> Option<u32>) -> bool {
    UPDATE.lock(|_| {
        if let Some(word) = change(current_word()) {
            publish(word);
            true
        } else {
            false
        }
    })
}

/// Keep a valid fault even when an EN reset reports ChipPowerOn.
pub fn boot() {
    let panic = Record::decode(panic_retained().load(Ordering::Acquire));
    let previous = state::select(
        retained().load(Ordering::Acquire),
        panic.map_or(0, Record::encode),
    );
    let word = previous.map_or(0, |record| record.after_boot().encode());
    PANIC_CURRENT.store(panic.map_or(0, Record::encode), Ordering::Release);
    publish(word);
    PREVIOUS.store(word, Ordering::Release);
}

pub fn blocked() -> bool {
    Record::decode(current_word()).is_some()
}

pub fn report_previous() {
    if let Some(previous) = Record::decode(PREVIOUS.load(Ordering::Acquire)) {
        log::error!(
            "display recovery: previous={} at {}; automatic refresh blocked",
            previous.status.label(),
            previous.stage.label()
        );
    }
}

pub fn connection_line() -> heapless::String<192> {
    let mut line = heapless::String::new();
    let current = Record::decode(current_word());
    let _ = writeln!(
        line,
        "INFO display recovery: status={}; stage={}; blocked={}",
        current.map_or("ready", |record| record.status.label()),
        current.map_or("none", |record| record.stage.label()),
        current.is_some(),
    );
    line
}

/// Progress records do not feed or extend the hardware deadline.
pub fn mark(stage: Stage) {
    let _ = update(|word| Record::decode(word).map(|record| record.progress(stage).encode()));
}

/// Record a shutdown failure without changing another watchdog owner's deadline.
pub fn latch_fault(stage: Stage) {
    let _ = update(|word| {
        Some(
            Record::decode(word)
                .map_or(
                    Record {
                        stage,
                        status: Status::Failed,
                    },
                    Record::failed,
                )
                .encode(),
        )
    });
}

/// This hook has no locks, logging, allocation, or hardware watchdog writes.
pub fn record_panic() {
    if let Some(record) = Record::decode(CURRENT.load(Ordering::Acquire))
        && record.status == Status::Active
    {
        let word = record.panicked().encode();
        // Separate storage prevents normal cleanup from erasing a concurrent panic.
        PANIC_CURRENT.store(word, Ordering::Release);
        panic_retained().store(word, Ordering::Release);
    }
}

#[derive(Debug)]
pub enum Error {
    Blocked,
    WatchdogBusy,
}

#[must_use = "keep the guard until display shutdown completes"]
pub struct Guard {
    finished: bool,
}

impl Guard {
    pub fn begin() -> Result<Self, Error> {
        if !update(|word| (word == 0).then_some(Record::beginning().encode())) {
            return Err(Error::Blocked);
        }
        if !recovery::arm(Owner::Display, state::timeout_ms()) {
            // No display control has changed. Keep the other owner's deadline.
            let _ = update(|word| (word == Record::beginning().encode()).then_some(0));
            return Err(Error::WatchdogBusy);
        }
        Ok(Self { finished: false })
    }

    /// Confirm controls-off readback and the discharge guard before release.
    /// A CPU reset cannot clear an expander output without a working I2C bus.
    pub fn finish(mut self, shutdown_confirmed: bool, success: bool) {
        let previous = Record::decode(current_word()).unwrap_or(Record::beginning().failed());
        let release_watchdog = previous
            .finish(shutdown_confirmed, success)
            .release_watchdog;
        let released = release_watchdog && recovery::disable(Owner::Display);
        let _ = update(|word| {
            let record = Record::decode(word).unwrap_or(Record::beginning().failed());
            if released {
                Some(
                    record
                        .finish(true, success)
                        .retained
                        .map_or(0, Record::encode),
                )
            } else {
                Some(record.failed().encode())
            }
        });
        self.finished = true;
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if !self.finished {
            // A returned error or panic cannot silently remove hardware protection.
            let _ = update(|word| {
                Some(
                    Record::decode(word)
                        .unwrap_or(Record::beginning())
                        .failed()
                        .encode(),
                )
            });
        }
    }
}

/// Use only after an explicit recovery request and confirmed display shutdown.
// Runtime currently requires a full power-off. Keep this gate for explicit recovery.
#[allow(dead_code)]
pub fn recover(shutdown_confirmed: bool) -> bool {
    if !shutdown_confirmed || !recovery::idle() {
        return false;
    }
    UPDATE.lock(|_| {
        if Record::decode(current_word()).is_some_and(|record| record.status == Status::Active) {
            return false;
        }
        // No active refresh can start while this lock protects the fault record.
        PANIC_CURRENT.store(0, Ordering::Release);
        panic_retained().store(0, Ordering::Release);
        publish(0);
        true
    })
}
