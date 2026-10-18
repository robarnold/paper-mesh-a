//! Deterministic scan fixtures. This adapter does not access host radios.

use paper_mesh_platform::discovery::{
    AddressKind, Discovery, Network, RadioKind, ScanCompletion, ScanError, ScanRequest,
    ScanResults, ScanSource,
};
use serde::{Deserialize, Serialize};

pub const SCAN_MS: u64 = 1_500;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanScenario {
    #[default]
    Nearby,
    Empty,
    Failed,
    Unavailable,
    Timeout,
}

#[derive(Default)]
pub struct SimDiscovery {
    pub scenario: ScanScenario,
    pending: Option<(ScanRequest, u64, ScanScenario)>,
}

impl SimDiscovery {
    pub fn next_deadline_ms(&self) -> Option<u64> {
        self.pending.and_then(|(_, ready, scenario)| {
            (!matches!(scenario, ScanScenario::Timeout)).then_some(ready)
        })
    }
}

impl Discovery for SimDiscovery {
    fn start(&mut self, request: ScanRequest, now_ms: u64) -> Result<ScanSource, ScanError> {
        if self.pending.is_some() {
            return Err(ScanError::Busy);
        }
        if matches!(self.scenario, ScanScenario::Unavailable) {
            return Err(ScanError::Unsupported);
        }
        self.pending = Some((request, now_ms.saturating_add(SCAN_MS), self.scenario));
        Ok(ScanSource::Simulated)
    }

    fn poll(&mut self, now_ms: u64, results: &mut ScanResults) -> Option<ScanCompletion> {
        let (request, ready, scenario) = self.pending?;
        if now_ms < ready || matches!(scenario, ScanScenario::Timeout) {
            return None;
        }
        self.pending = None;
        results.clear();
        let result = match scenario {
            ScanScenario::Nearby => {
                nearby(request.kind, results);
                Ok(())
            }
            ScanScenario::Empty => Ok(()),
            _ => Err(ScanError::Failed),
        };
        Some(ScanCompletion {
            request,
            observed_at_ms: ready,
            result,
        })
    }

    fn cancel(&mut self, request: ScanRequest) {
        if self
            .pending
            .is_some_and(|(pending, _, _)| pending == request)
        {
            self.pending = None;
        }
    }
}

fn nearby(kind: RadioKind, results: &mut ScanResults) {
    let names: [&[u8]; 7] = match kind {
        RadioKind::Wifi => [
            b"Workshop",
            b"Paper Mesh",
            b"",
            b"Guest",
            b"Paper Mesh",
            b"Garden",
            b"Distant AP",
        ],
        RadioKind::BluetoothLe => [
            b"Keyboard",
            b"Sensor",
            b"",
            b"Headphones",
            b"Beacon",
            b"Thermometer",
            b"Tracker",
        ],
    };
    for (index, (name, rssi)) in names
        .into_iter()
        .zip([
            Some(-73),
            Some(-38),
            Some(-64),
            Some(-55),
            Some(-48),
            Some(-89),
            None,
        ])
        .enumerate()
    {
        let address_kind = if kind == RadioKind::BluetoothLe && index % 2 == 0 {
            AddressKind::Random
        } else {
            AddressKind::Public
        };
        results.insert(Network::new(
            name,
            [0x02, 0, 0, 0, 0, index as u8 + 1],
            address_kind,
            rssi,
        ));
    }
}
