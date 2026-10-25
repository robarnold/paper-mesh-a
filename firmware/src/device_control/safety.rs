//! Conditions required before a USB request can reset the processor.

pub struct Check {
    pub connected: bool,
    pub bus_available: bool,
    pub display_in_flight: bool,
    pub display_blocked: bool,
    pub power_requested: bool,
    pub watchdog_idle: bool,
}

impl Check {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.connected {
            return Err("USB connection changed");
        }
        if !self.bus_available {
            return Err("Power controls unavailable");
        }
        if self.display_in_flight || self.power_requested || !self.watchdog_idle {
            return Err("Device is busy; wait for the current operation");
        }
        if self.display_blocked {
            return Err("Display recovery requires manual intervention");
        }
        Ok(())
    }
}
