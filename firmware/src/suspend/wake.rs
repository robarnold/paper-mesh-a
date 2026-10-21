//! Decode captured GPIO interrupts without an invented control identity.

use paper_mesh_platform::{input::buttons::ButtonId, suspend::WakeReason};

pub const BOOT: u32 = 1;
pub const EXPANDER: u32 = 2;
pub const TOUCH: u32 = 4;

pub fn reasons(captured: u32, port1: Option<u8>, timer: bool) -> [Option<WakeReason>; 4] {
    [
        timer.then_some(WakeReason::Timer),
        (captured & BOOT != 0).then_some(WakeReason::Control(ButtonId(0))),
        (captured & EXPANDER != 0).then_some(if port1.is_some_and(|value| value & 4 == 0) {
            WakeReason::Control(ButtonId(1))
        } else {
            WakeReason::UnidentifiedInput
        }),
        (captured & TOUCH != 0).then_some(WakeReason::TouchInterrupt),
    ]
}
