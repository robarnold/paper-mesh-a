//! Passive radio scans through esp-radio and the shared discovery contract.

mod advertisement;
mod bluetooth;
mod state;
mod wifi;

use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, Ordering};
use core::{cell::RefCell, future::Future, pin::Pin};
use embassy_futures::select::{Either3, select3};
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    signal::Signal,
};
use embassy_time::{Instant, Timer};
use esp_hal::peripherals::{BT, WIFI};
use paper_mesh_platform::discovery::{
    Discovery, Network, RadioKind, ScanCompletion, ScanError, ScanRequest, ScanResults, ScanSource,
};
use state::ScanState;

static STATE: Mutex<CriticalSectionRawMutex, RefCell<ScanState>> =
    Mutex::new(RefCell::new(ScanState::NEW));
static REQUEST: Signal<CriticalSectionRawMutex, Option<ScanRequest>> = Signal::new();
static CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::Acquire) || STATE.lock(|state| state.borrow().is_pending())
}

struct ActiveScan;
impl ActiveScan {
    fn new() -> Self {
        ACTIVE.store(true, Ordering::Release);
        Self
    }
}
impl Drop for ActiveScan {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Release);
    }
}

type ScanFuture<'a> = Pin<Box<dyn Future<Output = Result<(), ScanError>> + 'a>>;

pub struct FirmwareDiscovery;

impl Discovery for FirmwareDiscovery {
    fn start(&mut self, request: ScanRequest, now_ms: u64) -> Result<ScanSource, ScanError> {
        if crate::suspend::requested() {
            return Err(ScanError::Busy);
        }
        let source = STATE.lock(|state| state.borrow_mut().start(request, now_ms))?;
        REQUEST.signal(Some(request));
        Ok(source)
    }

    fn poll(&mut self, _: u64, results: &mut ScanResults) -> Option<ScanCompletion> {
        STATE.lock(|state| state.borrow_mut().poll(results))
    }

    fn cancel(&mut self, request: ScanRequest) {
        if STATE.lock(|state| state.borrow_mut().cancel(request)) {
            REQUEST.signal(None);
        }
    }
}

pub async fn changed() {
    CHANGED.wait().await;
}

fn observe(request: ScanRequest, network: Network) {
    STATE.lock(|state| state.borrow_mut().observe(request, network));
}

#[embassy_executor::task]
pub async fn worker(mut wifi: WIFI<'static>, mut bluetooth: BT<'static>) {
    let mut pending = None;
    loop {
        let next = match pending.take() {
            Some(request) => Some(request),
            None => REQUEST.wait().await,
        };
        let request = match next {
            Some(request) => request,
            None => continue,
        };
        if !STATE.lock(|state| state.borrow().accepts(request)) {
            continue;
        }
        let _active = ActiveScan::new();
        let result = if Instant::now().as_millis() >= request.deadline_ms {
            Err(ScanError::Timeout)
        } else {
            // The select operation drops the driver before the next request starts.
            match select3(
                REQUEST.wait(),
                Timer::at(Instant::from_millis(request.deadline_ms)),
                match request.kind {
                    RadioKind::Wifi => wifi::scan(wifi.reborrow(), request),
                    RadioKind::BluetoothLe => bluetooth::scan(bluetooth.reborrow(), request),
                },
            )
            .await
            {
                Either3::First(next) => {
                    pending = next;
                    continue;
                }
                Either3::Second(()) => Err(ScanError::Timeout),
                Either3::Third(result) => result,
            }
        };
        STATE.lock(|state| {
            state
                .borrow_mut()
                .finish(request, Instant::now().as_millis(), result)
        });
        CHANGED.signal(());
    }
}
