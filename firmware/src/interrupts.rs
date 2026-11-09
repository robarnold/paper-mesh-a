//! Board ownership of the shared GPIO interrupt vector.
//!
//! # Design choices
//!
//! - Install the GPIO vector independently of touch startup so one adapter does not own another client's delivery.
//! - Dispatch every registered client because several pins can become pending before one vector entry.
//!
//! See [interrupt ownership](../../docs/platform/runtime.md#interrupt-ownership).

use core::cell::Cell;
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use esp_hal::{gpio::Io, peripherals::IO_MUX, system::Cpu};
use paper_mesh_esp_adapters::interrupt::GpioInterruptHandler;

const GPIO_CLIENT_CAPACITY: usize = 2;
type Handlers = [Option<GpioInterruptHandler>; GPIO_CLIENT_CAPACITY];
static HANDLERS: Mutex<CriticalSectionRawMutex, Cell<Handlers>> =
    Mutex::new(Cell::new([None; GPIO_CLIENT_CAPACITY]));

/// Collect board registrations before enabling delivery. Registration order is dispatch order.
#[must_use]
pub struct Interrupts {
    io_mux: IO_MUX<'static>,
    handlers: Handlers,
    count: usize,
}

impl Interrupts {
    pub fn new(io_mux: IO_MUX<'static>) -> Self {
        Self {
            io_mux,
            handlers: [None; GPIO_CLIENT_CAPACITY],
            count: 0,
        }
    }

    pub fn register(&mut self, handler: GpioInterruptHandler) {
        assert!(
            self.count < GPIO_CLIENT_CAPACITY,
            "GPIO callback capacity exceeded"
        );
        self.handlers[self.count] = Some(handler);
        self.count += 1;
    }

    pub fn enable(self) {
        assert!(Cpu::current() == Cpu::ProCpu);
        HANDLERS.lock(|handlers| handlers.set(self.handlers));
        Io::new(self.io_mux).set_interrupt_handler(gpio_interrupt);
    }
}

#[esp_hal::handler]
fn gpio_interrupt() {
    // Copy registrations before callbacks so dispatch adds no critical section around client work.
    let handlers = HANDLERS.lock(Cell::get);
    for handler in handlers.into_iter().flatten() {
        handler.handle();
    }
}
