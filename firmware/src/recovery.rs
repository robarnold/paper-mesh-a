//! Share the RTC watchdog without replacing another operation's deadline.

mod state;
pub use state::Owner;

use core::cell::RefCell;
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use esp_hal::{
    peripherals::LPWR,
    rtc_cntl::{Rwdt, RwdtStage, RwdtStageAction},
    time::Duration,
};

struct HardwareWatchdog(Rwdt);

impl state::Watchdog for HardwareWatchdog {
    fn arm(&mut self, milliseconds: u64) {
        self.0.disable();
        self.0
            .set_timeout(RwdtStage::Stage0, Duration::from_millis(milliseconds));
        self.0.enable();
        self.0
            .set_stage_action(RwdtStage::Stage0, RwdtStageAction::ResetCore);
        // Keep RTC memory and run the deadline through a missing sleep wake event.
        let registers = LPWR::regs();
        registers
            .wdtwprotect()
            .write(|w| unsafe { w.bits(0x50d8_3aa1) });
        registers
            .wdtconfig0()
            .modify(|_, w| w.wdt_pause_in_slp().clear_bit());
        registers.wdtwprotect().write(|w| unsafe { w.bits(0) });
        self.0.feed();
    }

    fn disable(&mut self) {
        self.0.disable();
    }
}

type Manager = state::Manager<HardwareWatchdog>;
static MANAGER: Mutex<CriticalSectionRawMutex, RefCell<Option<Manager>>> =
    Mutex::new(RefCell::new(None));

pub fn initialize(mut watchdog: Rwdt) {
    MANAGER.lock(|manager| {
        let mut manager = manager.borrow_mut();
        assert!(manager.is_none(), "recovery watchdog initialized twice");
        watchdog.disable();
        *manager = Some(Manager::new(HardwareWatchdog(watchdog)));
    });
}

pub fn arm(owner: Owner, milliseconds: u64) -> bool {
    MANAGER.lock(|manager| {
        manager
            .borrow_mut()
            .as_mut()
            .is_some_and(|manager| manager.arm(owner, milliseconds))
    })
}

pub fn disable(owner: Owner) -> bool {
    MANAGER.lock(|manager| {
        manager
            .borrow_mut()
            .as_mut()
            .is_some_and(|manager| manager.disable(owner))
    })
}

/// Recovery can clear a retained display fault only when no deadline is active.
pub fn idle() -> bool {
    MANAGER.lock(|manager| manager.borrow().as_ref().is_some_and(Manager::idle))
}
