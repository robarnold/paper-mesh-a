//! Bounded observations for Wi-Fi and Bluetooth LE discovery.

use core::cmp::Ordering;
use heapless::{String, Vec};

pub const MAX_NETWORKS: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RadioKind {
    Wifi,
    BluetoothLe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AddressKind {
    Public,
    Random,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Network {
    pub name: String<32>,
    pub address: [u8; 6],
    pub address_kind: AddressKind,
    pub rssi_dbm: Option<i16>,
}

impl Network {
    /// This method limits names to 32 bytes of printable ASCII.
    /// Other bytes become question marks.
    /// An empty name identifies a hidden SSID or an unnamed Bluetooth device.
    pub fn new(
        name: &[u8],
        address: [u8; 6],
        address_kind: AddressKind,
        rssi_dbm: Option<i16>,
    ) -> Self {
        let mut label = String::new();
        for byte in name.iter().take(32) {
            let _ = label.push(if byte.is_ascii_graphic() || *byte == b' ' {
                *byte as char
            } else {
                '?'
            });
        }
        Self {
            name: label,
            address,
            address_kind,
            rssi_dbm,
        }
    }

    fn rank(&self, other: &Self) -> Ordering {
        other
            .rssi_dbm
            .cmp(&self.rssi_dbm)
            .then_with(|| self.address.cmp(&other.address))
            .then_with(|| self.address_kind.cmp(&other.address_kind))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanResults {
    networks: Vec<Network, MAX_NETWORKS>,
    pub truncated: bool,
}

impl ScanResults {
    pub const fn new() -> Self {
        Self {
            networks: Vec::new(),
            truncated: false,
        }
    }

    /// This method updates an address with its last report. It keeps up to 24 entries.
    /// Unknown RSSI follows measured RSSI. Equal values use address order.
    pub fn insert(&mut self, network: Network) {
        if let Some(index) = self.networks.iter().position(|old| {
            old.address == network.address && old.address_kind == network.address_kind
        }) {
            self.networks.remove(index);
        }
        if self.networks.is_full() {
            self.truncated = true;
            if network.rank(self.networks.last().unwrap()) != Ordering::Less {
                return;
            }
            self.networks.pop();
        }
        let _ = self.networks.push(network);
        self.networks.sort_unstable_by(Network::rank);
    }

    pub fn networks(&self) -> &[Network] {
        &self.networks
    }

    pub fn clear(&mut self) {
        self.networks.clear();
        self.truncated = false;
    }

    /// This method copies entries without a complete result buffer on the stack.
    pub fn copy_from(&mut self, other: &Self) {
        self.clear();
        for network in other.networks() {
            let _ = self.networks.push(network.clone());
        }
        self.truncated = other.truncated;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanSource {
    Simulated,
    Device,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanError {
    Unsupported,
    Busy,
    Failed,
    Timeout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanRequest {
    pub id: u64,
    pub kind: RadioKind,
    pub deadline_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanCompletion {
    pub request: ScanRequest,
    pub observed_at_ms: u64,
    pub result: Result<(), ScanError>,
}

/// The adapter controls discovery without connection, pairing, or application data transfer.
/// All times use the application clock. The consumer supplies storage for completed results.
/// A new application session must discard completions from the previous session.
pub trait Discovery {
    fn start(&mut self, request: ScanRequest, now_ms: u64) -> Result<ScanSource, ScanError>;
    /// This method replaces the buffer only when it returns a completion. Errors clear the buffer.
    fn poll(&mut self, now_ms: u64, results: &mut ScanResults) -> Option<ScanCompletion>;
    /// This method invalidates the request immediately. Resource release can finish asynchronously.
    /// The adapter releases resources before the next scan starts. The consumer rejects later completions for this request.
    fn cancel(&mut self, request: ScanRequest);
}

/// This adapter reports unsupported operations.
pub struct UnavailableDiscovery;

impl Discovery for UnavailableDiscovery {
    fn start(&mut self, _: ScanRequest, _: u64) -> Result<ScanSource, ScanError> {
        Err(ScanError::Unsupported)
    }

    fn poll(&mut self, _: u64, _: &mut ScanResults) -> Option<ScanCompletion> {
        None
    }

    fn cancel(&mut self, _: ScanRequest) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network(id: u8, rssi: Option<i16>) -> Network {
        Network::new(b"Same SSID", [0, 0, 0, 0, 0, id], AddressKind::Public, rssi)
    }

    #[test]
    fn sorts_signed_rssi_and_keeps_distinct_addresses() {
        let mut results = ScanResults::default();
        for (id, rssi) in [(4, None), (3, Some(-80)), (2, Some(-40)), (1, Some(-40))] {
            results.insert(network(id, rssi));
        }
        assert_eq!(
            results
                .networks()
                .iter()
                .map(|n| n.address[5])
                .collect::<Vec<_, 4>>()
                .as_slice(),
            &[1, 2, 3, 4]
        );
        results.insert(network(3, Some(-20)));
        assert_eq!(results.networks().len(), 4);
        assert_eq!(results.networks()[0].address[5], 3);
        let mut random = network(3, None);
        random.address_kind = AddressKind::Random;
        results.insert(random);
        assert_eq!(results.networks().len(), 5);
    }

    #[test]
    fn limits_results_and_reports_overflow() {
        let mut results = ScanResults::default();
        for id in 0..30 {
            results.insert(network(id, Some(-100 + i16::from(id))));
        }
        assert!(results.truncated);
        assert_eq!(results.networks().len(), MAX_NETWORKS);
        assert_eq!(results.networks()[0].rssi_dbm, Some(-71));
        assert_eq!(results.networks().last().unwrap().rssi_dbm, Some(-94));
        assert_eq!(
            Network::new(b"A\n\xff", [0; 6], AddressKind::Public, None).name,
            "A??"
        );
    }
}
