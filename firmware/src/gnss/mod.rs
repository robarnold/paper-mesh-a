//! GNSS UART input and continuous auxiliary power for H752-01.

mod gsv;
mod power;
mod receiver;

use core::cell::RefCell;
use embassy_futures::select::{Either, select};
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    signal::Signal,
};
use embassy_time::{Instant, Timer};
use esp_hal::{
    Blocking,
    interrupt::Priority,
    peripherals,
    uart::{Config, RxConfig, Uart},
};
use esp_rtos::embassy::InterruptExecutor;
use log::{error, info};
use paper_mesh_platform::location::{Location, LocationAvailability, LocationSnapshot};
use receiver::Receiver;
use static_cell::{ConstStaticCell, StaticCell};

use crate::bus::Bus;

static EXECUTOR: StaticCell<InterruptExecutor<3>> = StaticCell::new();
static RECEIVER: ConstStaticCell<Receiver> = ConstStaticCell::new(Receiver::INITIAL);
static STATE: Mutex<CriticalSectionRawMutex, RefCell<LocationSnapshot>> =
    Mutex::new(RefCell::new(LocationSnapshot::new()));
static CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

pub struct FirmwareLocation;

impl Location for FirmwareLocation {
    fn snapshot(&mut self, _now_ms: u64) -> LocationSnapshot {
        STATE.lock(|state| *state.borrow())
    }
}

pub async fn changed() {
    CHANGED.wait().await;
}

fn publish(snapshot: LocationSnapshot) {
    STATE.lock(|state| *state.borrow_mut() = snapshot);
    CHANGED.signal(());
}

fn config(baud: u32) -> Config {
    Config::default().with_baudrate(baud).with_rx(
        RxConfig::default()
            .with_fifo_full_threshold(32)
            .with_timeout(10),
    )
}

pub fn start(
    bus: &'static Bus,
    uart: peripherals::UART1<'static>,
    rx: peripherals::GPIO44<'static>,
    tx: peripherals::GPIO43<'static>,
    interrupt: peripherals::FROM_CPU_INTR3<'static>,
) {
    let uart = match Uart::new(uart, config(9_600)) {
        Ok(uart) => uart.with_rx(rx).with_tx(tx),
        Err(error) => {
            error!("GNSS: UART configuration failed: {:?}", error);
            fault();
            return;
        }
    };
    // The interrupt executor services UART input during application rendering.
    EXECUTOR
        .init(InterruptExecutor::new(interrupt))
        .start(Priority::Priority1)
        .spawn(worker(bus, uart).unwrap());
}

pub fn fault() {
    publish(LocationSnapshot {
        availability: LocationAvailability::Fault,
        ..LocationSnapshot::new()
    });
}

async fn enable_power(bus: &Bus) -> Result<(), esp_hal::i2c::master::Error> {
    let mut bus = bus.lock().await;
    power::enable(&mut *bus)
}

#[embassy_executor::task]
async fn worker(bus: &'static Bus, uart: Uart<'static, Blocking>) {
    let mut uart = uart.into_async();
    loop {
        match enable_power(bus).await {
            Ok(()) => break,
            Err(error) => {
                error!("GNSS: supply enable failed: {:?}", error);
                fault();
                Timer::after_secs(1).await;
            }
        }
    }
    info!("GNSS: auxiliary supply enabled; core=0; RX44/TX43; continuous power");
    // The GSV groups use static RAM instead of the interrupt stack.
    let receiver = RECEIVER.take();
    receiver.start_at(Instant::now().as_millis());
    publish(receiver.snapshot);
    let mut bytes = [0; 128];
    loop {
        let mut changed = false;
        match select(
            uart.read_async(&mut bytes),
            Timer::at(Instant::from_millis(receiver.deadline_ms())),
        )
        .await
        {
            Either::First(Ok(length)) => {
                let was_ready = receiver.snapshot.availability == LocationAvailability::Ready;
                let now = Instant::now().as_millis();
                for byte in &bytes[..length] {
                    changed |= receiver.feed(*byte, now);
                }
                if changed && !was_ready {
                    info!("GNSS: NMEA input at {} baud", receiver.baud());
                }
            }
            Either::First(Err(error)) => {
                // A baud probe can cause framing errors. Log only a failure after valid input.
                if receiver.snapshot.availability == LocationAvailability::Ready {
                    error!("GNSS: UART input failed: {:?}", error);
                }
                receiver.uart_error();
                // Reconfiguration clears the FIFO after an error.
                if uart.apply_config(&config(receiver.baud())).is_err() {
                    fault();
                    return;
                }
                changed = true;
            }
            Either::Second(()) => {}
        }
        // Check the deadline after input too. Noise must not prevent a baud change.
        let previous_baud = receiver.baud();
        if receiver.advance(Instant::now().as_millis()) {
            if receiver.baud() != previous_baud {
                info!("GNSS: probe at {} baud", receiver.baud());
                if uart.apply_config(&config(receiver.baud())).is_err() {
                    fault();
                    return;
                }
            }
            changed = true;
        }
        if changed {
            publish(receiver.snapshot);
        }
    }
}
