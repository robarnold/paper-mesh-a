use esp_hal::{peripherals::WIFI, time::Duration};
use esp_radio::wifi::{
    WifiController,
    scan::{ScanConfig, ScanTypeConfig},
};
use paper_mesh_platform::discovery::{AddressKind, MAX_NETWORKS, Network, ScanError, ScanRequest};

pub fn scan(device: WIFI<'_>, request: ScanRequest) -> super::ScanFuture<'_> {
    Box::pin(run(device, request))
}

fn controller(device: WIFI<'_>) -> Result<WifiController<'_>, ScanError> {
    WifiController::new(device, Default::default()).map_err(failed)
}

async fn run(device: WIFI<'_>, request: ScanRequest) -> Result<(), ScanError> {
    let mut controller = controller(device)?;
    let config = scan_config();
    let access_points = controller.scan_async(&config).await.map_err(failed)?;
    for ap in access_points {
        let mut network = Network::new(
            ap.ssid.as_str().as_bytes(),
            ap.bssid,
            AddressKind::Public,
            Some(i16::from(ap.signal_strength)),
        );
        // The driver exposes only the valid UTF-8 prefix. Mark the unavailable suffix.
        for _ in network.name.len()..ap.ssid.len() {
            let _ = network.name.push('?');
        }
        super::observe(request, network);
    }
    // Controller destruction stops Wi-Fi on success, error, timeout, and cancellation.
    Ok(())
}

fn scan_config() -> ScanConfig {
    ScanConfig::default()
        .with_show_hidden(true)
        .with_scan_type(ScanTypeConfig::Passive(Duration::from_millis(120)))
        .with_max(MAX_NETWORKS + 1)
}

fn failed(error: esp_radio::wifi::WifiError) -> ScanError {
    log::warn!("Wi-Fi scan failed: {:?}", error);
    ScanError::Failed
}
use alloc::boxed::Box;
