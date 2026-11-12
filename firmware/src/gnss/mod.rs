//! GNSS UART input and continuous auxiliary power for H752-01.
//!
//! # Design choices
//!
//! - Restart receiver acquisition after auxiliary power returns so pre-sleep data
//!   cannot masquerade as a new fix.
//!
//! See [Transport and power](../../../docs/boards/lilygo-t5-s3-pro-gnss.md#transport-and-power).

mod config;
pub use config::{BaudRates, Config};
mod gsv;
mod lifecycle;
pub use lifecycle::{GnssLifecycle, PreparationError};
mod power;
mod receiver;

use core::cell::{Cell, RefCell};
use embassy_executor::SendSpawner;
use embassy_futures::select::{Either, select};
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    signal::Signal,
};
use embassy_time::{Instant, Timer};
use esp_hal::{
    Blocking, peripherals,
    uart::{Config as UartConfig, RxConfig, Uart, UartTx},
};
use log::{error, info};
use paper_mesh_platform::location::{Location, LocationAvailability, LocationSnapshot};
use paper_mesh_platform::worker::{Pause, PauseWorker};
use receiver::{Receiver, ReceiverStorage};
use static_cell::{ConstStaticCell, StaticCell};

use crate::bus::Bus;

static RECEIVER_STORAGE: ConstStaticCell<ReceiverStorage> =
    ConstStaticCell::new(ReceiverStorage::EMPTY);
static STATE: Mutex<CriticalSectionRawMutex, RefCell<LocationSnapshot>> =
    Mutex::new(RefCell::new(LocationSnapshot::new()));
static CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static PAUSE: StaticCell<Pause<CriticalSectionRawMutex>> = StaticCell::new();
static PREPARATION: lifecycle::PreparationReport = Mutex::new(Cell::new(Ok(())));

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

fn uart_config(baud: u32) -> UartConfig {
    UartConfig::default().with_baudrate(baud).with_rx(
        RxConfig::default()
            .with_fifo_full_threshold(32)
            .with_timeout(10),
    )
}

pub fn start(
    spawner: SendSpawner,
    bus: &'static Bus,
    uart: peripherals::UART1<'static>,
    rx: peripherals::GPIO44<'static>,
    tx: peripherals::GPIO43<'static>,
    config: Config,
) -> GnssLifecycle {
    let (pause, worker_pause) = PAUSE.init(Pause::new()).split();
    spawner.spawn(worker(bus, uart, rx, tx, worker_pause, config).unwrap());
    GnssLifecycle {
        pause,
        report: &PREPARATION,
    }
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
async fn worker(
    bus: &'static Bus,
    mut peripheral: peripherals::UART1<'static>,
    mut rx: peripherals::GPIO44<'static>,
    mut tx: peripherals::GPIO43<'static>,
    mut pause: PauseWorker<'static, CriticalSectionRawMutex>,
    config: Config,
) {
    let mut receiver = Receiver::new(RECEIVER_STORAGE.take(), config);
    loop {
        if pause.is_requested() {
            // HAL sleep polls every UART transmitter. Keep UART1 clocked after RX drops,
            // or that poll cannot finish. This idle driver has no pins or RX wake lock.
            let idle_uart = UartTx::new(
                peripheral.reborrow(),
                uart_config(config.baud_rates.first.get()),
            );
            let result = {
                let mut bus = bus.lock().await;
                power::disable(&mut *bus)
            };
            receiver.uart_error();
            publish(receiver.snapshot);
            lifecycle::publish_preparation(&PREPARATION, idle_uart.is_ok(), result.is_ok());
            if let Err(error) = PREPARATION.lock(Cell::get) {
                error!("GNSS: preparation failed: {:?}", error);
            }
            pause.checkpoint().await;
            drop(idle_uart);
        }
        if let Err(error) = enable_power(bus).await {
            error!("GNSS: supply enable failed: {:?}", error);
            fault();
            select(
                Timer::after_millis(config.retry_ms.get()),
                pause.requested(),
            )
            .await;
            continue;
        }
        let uart = match Uart::new(
            peripheral.reborrow(),
            uart_config(config.baud_rates.first.get()),
        ) {
            Ok(uart) => uart.with_rx(rx.reborrow()).with_tx(tx.reborrow()),
            Err(error) => {
                error!("GNSS: UART configuration failed: {:?}", error);
                fault();
                select(
                    Timer::after_millis(config.retry_ms.get()),
                    pause.requested(),
                )
                .await;
                continue;
            }
        };
        receiver.reset();
        info!("GNSS: auxiliary supply enabled; core=0; RX44/TX43");
        select(pause.requested(), receive(uart, &mut receiver)).await;
        // Cancellation drops UART RX and its wake lock before the pause acknowledgement.
    }
}

async fn receive(uart: Uart<'_, Blocking>, receiver: &mut Receiver<'_>) {
    let mut uart = uart.into_async();
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
                if uart.apply_config(&uart_config(receiver.baud())).is_err() {
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
                if uart.apply_config(&uart_config(receiver.baud())).is_err() {
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
