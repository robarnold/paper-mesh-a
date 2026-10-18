//! Native tests use the same state and packet conversion code as the firmware.

#[path = "../../firmware/src/discovery/advertisement.rs"]
mod advertisement;
#[path = "../../firmware/src/discovery/state.rs"]
mod state;

use paper_mesh_app::platform::discovery::*;
use state::ScanState;

fn request(id: u64, kind: RadioKind) -> ScanRequest {
    ScanRequest {
        id,
        kind,
        deadline_ms: 10_000,
    }
}

fn network(id: u8, name: &[u8], rssi: Option<i16>) -> Network {
    Network::new(name, [0, 0, 0, 0, 0, id], AddressKind::Public, rssi)
}

fn report(address_kind: u8, address: [u8; 6], data: &[u8], rssi: i8) -> Vec<u8> {
    let mut bytes = vec![0x02, 1, 0, address_kind];
    bytes.extend_from_slice(&address);
    bytes.push(data.len() as u8);
    bytes.extend_from_slice(data);
    bytes.push(rssi as u8);
    bytes
}

#[test]
fn cancellation_rejects_old_reports_and_completions_after_radio_change() {
    let mut state = ScanState::NEW;
    let wifi = request(1, RadioKind::Wifi);
    let ble = request(2, RadioKind::BluetoothLe);
    assert_eq!(state.start(wifi, 0), Ok(ScanSource::Device));
    state.observe(wifi, network(1, b"old", Some(-10)));
    assert!(state.cancel(wifi));
    assert_eq!(state.start(ble, 1), Ok(ScanSource::Device));
    assert!(!state.cancel(wifi));
    state.observe(wifi, network(2, b"late", Some(-5)));
    state.finish(wifi, 2, Ok(()));
    state.observe(ble, network(3, b"current", Some(-50)));
    state.finish(ble, 3, Ok(()));
    let mut results = ScanResults::new();
    let completion = state.poll(&mut results).unwrap();
    assert_eq!(completion.request, ble);
    assert_eq!(results.networks(), &[network(3, b"current", Some(-50))]);
    assert!(state.poll(&mut results).is_none());
}

#[test]
fn unfinished_and_unconsumed_requests_reject_a_second_start() {
    let mut state = ScanState::NEW;
    let first = request(1, RadioKind::Wifi);
    let second = request(2, RadioKind::BluetoothLe);
    state.start(first, 0).unwrap();
    assert_eq!(state.start(second, 0), Err(ScanError::Busy));
    state.finish(first, 100, Ok(()));
    assert_eq!(state.start(second, 100), Err(ScanError::Busy));
    assert!(state.cancel(first));
    assert!(state.poll(&mut ScanResults::new()).is_none());
    assert!(state.start(second, 100).is_ok());
}

#[test]
fn failures_and_expired_requests_discard_partial_results() {
    let mut state = ScanState::NEW;
    let request = request(1, RadioKind::Wifi);
    assert_eq!(
        state.start(request, request.deadline_ms),
        Err(ScanError::Timeout)
    );
    let mut results = ScanResults::new();
    for (time, outcome, expected) in [
        (50, Err(ScanError::Failed), Err(ScanError::Failed)),
        (10_001, Ok(()), Err(ScanError::Timeout)),
        (50, Ok(()), Ok(())),
    ] {
        state.start(request, 0).unwrap();
        if outcome.is_err() || time > request.deadline_ms {
            state.observe(request, network(1, b"partial", Some(-30)));
        }
        results.insert(network(2, b"previous scan", Some(-20)));
        state.finish(request, time, outcome);
        assert_eq!(state.poll(&mut results).unwrap().result, expected);
        assert!(results.networks().is_empty());
    }
}

#[test]
fn latest_rssi_preserves_names_and_the_result_limit() {
    let mut state = ScanState::NEW;
    let request = request(1, RadioKind::BluetoothLe);
    state.start(request, 0).unwrap();
    for id in 0..30 {
        state.observe(request, network(id, b"sensor", Some(-100 + i16::from(id))));
    }
    state.observe(request, network(29, b"", Some(-110)));
    state.observe(request, network(28, b"", None));
    state.finish(request, 50, Ok(()));
    let mut results = ScanResults::new();
    state.poll(&mut results).unwrap();
    assert!(results.truncated);
    assert_eq!(results.networks().len(), MAX_NETWORKS);
    assert_eq!(results.networks()[0].address[5], 27);
    assert_eq!(
        results.networks()[MAX_NETWORKS - 2],
        network(29, b"sensor", Some(-110))
    );
    assert_eq!(
        results.networks().last(),
        Some(&network(28, b"sensor", None))
    );
}

#[test]
fn ble_reports_convert_name_address_kind_byte_order_and_unknown_rssi() {
    let data = [2, 0x08, b'S', 5, 0x09, b'N', b'a', b'm', b'e'];
    let public = report(0, [6, 5, 4, 3, 2, 1], &data, -42);
    let random = report(1, [6, 5, 4, 3, 2, 1], &[], 127);
    let mut combined = public.clone();
    combined[1] = 2;
    combined.extend_from_slice(&random[2..]);
    let mut results = ScanResults::new();
    advertisement::visit(&combined, |network| results.insert(network)).unwrap();
    assert_eq!(results.networks().len(), 2);
    assert_eq!(results.networks()[0].name, "Name");
    assert_eq!(results.networks()[0].address, [1, 2, 3, 4, 5, 6]);
    assert_eq!(results.networks()[0].rssi_dbm, Some(-42));
    assert_eq!(results.networks()[1].address_kind, AddressKind::Random);
    assert_eq!(results.networks()[1].rssi_dbm, None);
}

#[test]
fn malformed_advertising_data_keeps_address_and_rssi_without_a_name() {
    let bytes = report(0, [0; 6], &[20, 0x09, b'N'], -80);
    let mut results = ScanResults::new();
    advertisement::visit(&bytes, |network| results.insert(network)).unwrap();
    assert_eq!(results.networks(), &[network(0, b"", Some(-80))]);
}

#[test]
fn truncated_hci_reports_and_invalid_values_return_errors() {
    let bytes = report(0, [0; 6], &[2, 0x09, b'N'], -80);
    for length in 0..bytes.len() {
        assert_eq!(
            advertisement::visit(&bytes[..length], |_| {}),
            Err(ScanError::Failed)
        );
    }
    assert_eq!(
        advertisement::visit(&report(255, [0; 6], &[], -40), |_| {}),
        Err(ScanError::Failed)
    );
    assert_eq!(
        advertisement::visit(&report(0, [0; 6], &[], -128), |_| {}),
        Err(ScanError::Failed)
    );
}
