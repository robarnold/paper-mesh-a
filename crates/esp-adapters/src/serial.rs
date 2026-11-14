//! ESP UART ownership, receive sessions, and ESP sleep preparation.
//!
//! # Design choices
//!
//! - Rebuild the UART for each session to clear stale input without exposing ESP details to GNSS.
//! - Keep the pinless idle transmitter separate from receive-session cleanup because sleep preparation can fail.
//!
//! See [serial sessions](../../../docs/adapters/serial.md).

use core::num::NonZeroU32;
use esp_hal::{
    Async, Blocking,
    gpio::{AnyPin, InputPin, OutputPin},
    uart::{AnyUart, Config, ConfigError, Uart, UartTx},
};
use paper_mesh_platform::serial::SerialReceiver;

/// ESP UART connection with runtime-selected peripheral, pins, and settings.
pub struct Serial {
    uart: AnyUart<'static>,
    rx: AnyPin<'static>,
    tx: AnyPin<'static>,
    config: Config,
}

impl Serial {
    /// Retain resources without configuring hardware. Firmware must select pins available on its board.
    pub fn new(
        uart: impl Into<AnyUart<'static>>,
        rx: impl InputPin + 'static,
        tx: impl OutputPin + 'static,
        config: Config,
    ) -> Self {
        Self {
            uart: uart.into(),
            rx: rx.degrade(),
            tx: tx.degrade(),
            config,
        }
    }

    /// Hold the selected UART clocked while the HAL checks transmitters during sleep preparation.
    /// The exclusive borrow proves no receive session remains. Dropping this guard permits another session.
    pub fn prepare_sleep(&mut self) -> Result<UartTx<'_, Blocking>, ConfigError> {
        UartTx::new(self.uart.reborrow(), self.config)
    }
}

impl SerialReceiver for Serial {
    type OpenError = ConfigError;
    type Session<'a>
        = Uart<'a, Async>
    where
        Self: 'a;

    fn open(&mut self, baud: NonZeroU32) -> Result<Self::Session<'_>, Self::OpenError> {
        // HAL initialization resets both FIFOs. Dropping RX releases its wake lock.
        Ok(
            Uart::new(self.uart.reborrow(), self.config.with_baudrate(baud.get()))?
                .with_rx(self.rx.reborrow())
                .with_tx(self.tx.reborrow())
                .into_async(),
        )
    }
}
