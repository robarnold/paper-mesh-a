//! Deterministic host connection and report observations.

use paper_mesh_platform::touchpad::{HostFrame, HostState, TouchpadHost};

pub struct SimTouchpad {
    pub state: HostState,
    pub active: bool,
    pub reports: u64,
    pub detachments: u64,
    pub last: Option<HostFrame>,
}

impl Default for SimTouchpad {
    fn default() -> Self {
        Self {
            state: HostState::Ready,
            active: false,
            reports: 0,
            detachments: 0,
            last: None,
        }
    }
}

impl TouchpadHost for SimTouchpad {
    fn generation(&self) -> u32 {
        self.detachments as u32
    }
    fn state(&self) -> HostState {
        self.state
    }
    fn set_active(&mut self, active: bool) {
        self.active = active;
        if !active {
            self.cancel();
        }
    }
    fn cancel(&mut self) {
        self.detachments += 1;
        self.last = None;
    }
    fn submit(&mut self, frame: HostFrame) -> bool {
        if !self.active || self.state != HostState::Ready {
            return false;
        }
        self.last = Some(frame);
        self.reports += 1;
        true
    }
}
