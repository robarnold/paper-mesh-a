//! Recover an interrupted suspend experiment without resetting RTC memory.

use esp_hal::{
    peripherals::LPWR,
    rtc_cntl::{Rwdt, RwdtStage, RwdtStageAction},
    time::Duration,
};

pub struct Recovery(Rwdt);

impl Recovery {
    pub fn new(watchdog: Rwdt) -> Self {
        Self(watchdog)
    }

    pub fn arm(&mut self, milliseconds: u64) {
        self.0.disable();
        self.0
            .set_timeout(RwdtStage::Stage0, Duration::from_millis(milliseconds));
        self.0.enable();
        self.0
            .set_stage_action(RwdtStage::Stage0, RwdtStageAction::ResetCore);
        // The recovery deadline must also cover a missing sleep wake event.
        // This coordinator is the only RTC watchdog owner.
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

    pub fn disable(&mut self) {
        self.0.disable();
    }
}
