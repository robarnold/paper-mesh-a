//! Optional diagnostic scans before the application loop starts.

use embassy_time::{Instant, Timer};
use paper_mesh_platform::discovery::{
    Discovery, RadioKind, ScanCompletion, ScanRequest, ScanResults,
};
use static_cell::ConstStaticCell;

const SCAN_TIMEOUT_MS: u64 = 10_000;

static RESULTS: ConstStaticCell<ScanResults> = ConstStaticCell::new(ScanResults::new());

pub async fn run(discovery: &mut impl Discovery) {
    let results = RESULTS.take();
    for (id, kind) in [(1, RadioKind::Wifi), (2, RadioKind::BluetoothLe)] {
        let now_ms = Instant::now().as_millis();
        let request = ScanRequest {
            id,
            kind,
            deadline_ms: now_ms + SCAN_TIMEOUT_MS,
        };
        if let Err(error) = discovery.start(request, now_ms) {
            log::warn!("scan {:?}: {:?}", kind, error);
            continue;
        }
        loop {
            let now_ms = Instant::now().as_millis();
            if let Some(completion) = discovery.poll(now_ms, results) {
                show(completion, results);
                break;
            }
            if now_ms >= request.deadline_ms {
                discovery.cancel(request);
                log::warn!("scan {:?}: timeout", kind);
                break;
            }
            Timer::after_millis(20).await;
        }
    }
}

fn show(completion: ScanCompletion, results: &ScanResults) {
    log::info!(
        "scan {:?}: {:?}, truncated={}",
        completion.request.kind,
        completion.result,
        results.truncated
    );
    for network in results.networks() {
        log::info!(
            "{} {:02x?} {:?} RSSI={:?} dBm",
            network.name,
            network.address,
            network.address_kind,
            network.rssi_dbm
        );
    }
}
