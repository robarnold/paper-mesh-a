//! Adapter callbacks for a shared GPIO interrupt source.
//!
//! # Design choices
//!
//! - Describe callbacks without installing a vector so firmware owns core routing and dispatch order.
//! - Keep GPIO callbacks separate from HAL-managed peripheral interrupts and executor software interrupts.
//!
//! See [interrupt ownership](../../../docs/platform/runtime.md#interrupt-ownership).

/// One client's callback for the shared GPIO interrupt source.
/// The callback must check its own pending pins and acknowledge only events it owns.
/// It runs in interrupt context and must not wait or perform device transactions.
#[derive(Clone, Copy)]
pub struct GpioInterruptHandler {
    callback: fn(),
}

impl GpioInterruptHandler {
    pub const fn new(callback: fn()) -> Self {
        Self { callback }
    }

    /// Dispatch even when another client caused the interrupt. The callback must tolerate no pending event.
    pub fn handle(&self) {
        (self.callback)();
    }
}
