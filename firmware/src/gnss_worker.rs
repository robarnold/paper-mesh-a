//! Firmware composition of GNSS sessions, UART preparation, and rail admission.
//!
//! # Design choices
//!
//! - Retain the detected baud across suspend and ordinary receive retries.
//! - Close every session before preparing ESP UART hardware and acknowledging a pause.
//!
//! See [GNSS coordination](../../docs/platform/worker-coordination.md#gnss-integration).

mod lifecycle;
pub use lifecycle::{GnssLifecycle, PreparationError};

use crate::{gnss, serial::Serial};
use core::{cell::Cell, num::NonZeroU64};
use embassy_executor::{SendSpawner, SpawnError};
use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
use embassy_time::{Instant, Timer};
use paper_mesh_gnss::{Receiver, ReceiverStorage, acquisition, probe};
use paper_mesh_platform::{
    serial::SerialReceiver,
    worker::{Pause, PauseWorker},
};
use static_cell::{ConstStaticCell, StaticCell};

static STORAGE: ConstStaticCell<ReceiverStorage> = ConstStaticCell::new(ReceiverStorage::EMPTY);
static PAUSE: StaticCell<Pause<CriticalSectionRawMutex>> = StaticCell::new();
static PREPARATION: lifecycle::PreparationReport = Mutex::new(Cell::new(Ok(())));

/// Runtime probe limits and retry policy, separate from observation decoding.
#[derive(Clone, Copy)]
pub struct Config {
    pub rates: probe::BaudRates,
    pub probe_ms: NonZeroU64,
    pub retry_ms: NonZeroU64,
    pub receiver: paper_mesh_gnss::Config,
}

/// Prepared task resources and their matching lifecycle control.
#[must_use]
pub struct Prepared {
    pub workers: Workers,
    pub lifecycle: GnssLifecycle,
}

/// Concrete task resources launched on the firmware-selected executor.
#[must_use]
pub struct Workers {
    serial: Serial,
    config: Config,
    supply: PauseWorker<'static, CriticalSectionRawMutex>,
    pause: PauseWorker<'static, CriticalSectionRawMutex>,
}

impl Workers {
    pub fn spawn(self, spawner: SendSpawner) -> Result<(), SpawnError> {
        spawner.spawn(worker(self)?);
        Ok(())
    }
}

pub fn prepare(
    serial: Serial,
    config: Config,
    supply: PauseWorker<'static, CriticalSectionRawMutex>,
) -> Prepared {
    let (pause, worker_pause) = PAUSE.init(Pause::new()).split();
    Prepared {
        workers: Workers {
            serial,
            config,
            supply,
            pause: worker_pause,
        },
        lifecycle: GnssLifecycle {
            pause,
            report: &PREPARATION,
        },
    }
}

#[embassy_executor::task]
async fn worker(workers: Workers) {
    let Workers {
        mut serial,
        config,
        mut supply,
        mut pause,
    } = workers;
    let mut receiver = Receiver::new(STORAGE.take(), config.receiver);
    let mut baud = None;
    loop {
        if pause.is_requested() {
            prepare_pause(&mut serial, &mut receiver, &mut pause).await;
        }
        // Rail startup may fail. Still handle suspend so UART preparation can acknowledge its pause.
        if matches!(
            select(pause.requested(), supply.checkpoint()).await,
            Either::First(())
        ) {
            continue;
        }
        if baud.is_none() {
            baud = probe_startup(&mut serial, config, &mut pause).await;
            if baud.is_none() {
                continue;
            }
        }
        receive_session(&mut serial, baud.unwrap(), &mut receiver, &mut pause).await;
        // The session and RX wake lock are gone before any pause acknowledgement.
        select(
            pause.requested(),
            Timer::after_millis(config.retry_ms.get()),
        )
        .await;
    }
}

// Separate the receive transition from preparation and probing to bound coordinator stack use.
async fn receive_session(
    serial: &mut Serial,
    baud: core::num::NonZeroU32,
    receiver: &mut Receiver<'_>,
    pause: &mut PauseWorker<'static, CriticalSectionRawMutex>,
) {
    let mut session = match serial.open(baud) {
        Ok(session) => session,
        Err(error) => {
            log::error!("GNSS session open failed: {:?}", error);
            gnss::fault();
            return;
        }
    };
    if let Either::Second(end) = select(
        pause.requested(),
        acquisition::run(
            &mut session,
            receiver,
            || Instant::now().as_millis(),
            |ms| Timer::at(Instant::from_millis(ms)),
            gnss::publish,
        ),
    )
    .await
    {
        log::warn!("GNSS session ended: {:?}", end);
    }
}

// Keep probe construction outside the coordinator's preparation frame.
async fn probe_startup(
    serial: &mut Serial,
    config: Config,
    pause: &mut PauseWorker<'static, CriticalSectionRawMutex>,
) -> Option<core::num::NonZeroU32> {
    match select(
        pause.requested(),
        probe::find(serial, config.rates, || {
            Timer::after_millis(config.probe_ms.get())
        }),
    )
    .await
    {
        Either::First(()) => None,
        Either::Second(Ok(baud)) => {
            log::info!("GNSS: detected {} baud", baud);
            Some(baud)
        }
        Either::Second(Err(error)) => {
            log::warn!("GNSS probe failed: {:?}", error);
            gnss::fault();
            select(
                pause.requested(),
                Timer::after_millis(config.retry_ms.get()),
            )
            .await;
            None
        }
    }
}

// Preparation owns the serial sleep guard until the lifecycle controller releases the pause.
async fn prepare_pause(
    serial: &mut Serial,
    receiver: &mut Receiver<'_>,
    pause: &mut PauseWorker<'static, CriticalSectionRawMutex>,
) {
    receiver.uart_error();
    gnss::publish(receiver.snapshot);
    let prepared = serial.prepare_sleep();
    lifecycle::publish_preparation(&PREPARATION, prepared.is_ok());
    if let Err(error) = &prepared {
        log::error!("GNSS transport preparation failed: {:?}", error);
    }
    pause.checkpoint().await;
    drop(prepared);
}
