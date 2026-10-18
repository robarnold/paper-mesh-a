//! Request identity and bounded storage for the radio worker.

use paper_mesh_app::platform::discovery::{
    Network, ScanCompletion, ScanError, ScanRequest, ScanResults, ScanSource,
};

pub struct ScanState {
    active: Option<ScanRequest>,
    completion: Option<ScanCompletion>,
    results: ScanResults,
}

impl ScanState {
    pub const NEW: Self = Self {
        active: None,
        completion: None,
        results: ScanResults::new(),
    };

    pub fn start(&mut self, request: ScanRequest, now_ms: u64) -> Result<ScanSource, ScanError> {
        if self.active.is_some() {
            return Err(ScanError::Busy);
        }
        if now_ms >= request.deadline_ms {
            return Err(ScanError::Timeout);
        }
        self.results.clear();
        self.completion = None;
        self.active = Some(request);
        Ok(ScanSource::Device)
    }

    pub fn accepts(&self, request: ScanRequest) -> bool {
        self.active == Some(request) && self.completion.is_none()
    }

    pub fn observe(&mut self, request: ScanRequest, mut network: Network) {
        if !self.accepts(request) {
            return;
        }
        // Some advertisements omit the name. Keep the name from the same scan and address.
        if network.name.is_empty()
            && let Some(previous) = self.results.networks().iter().find(|old| {
                old.address == network.address && old.address_kind == network.address_kind
            })
        {
            network.name.clone_from(&previous.name);
        }
        self.results.insert(network);
    }

    pub fn finish(&mut self, request: ScanRequest, now_ms: u64, result: Result<(), ScanError>) {
        if !self.accepts(request) {
            return;
        }
        let result = if now_ms > request.deadline_ms {
            Err(ScanError::Timeout)
        } else {
            result
        };
        if result.is_err() {
            self.results.clear();
        }
        self.completion = Some(ScanCompletion {
            request,
            observed_at_ms: now_ms,
            result,
        });
    }

    pub fn poll(&mut self, results: &mut ScanResults) -> Option<ScanCompletion> {
        let completion = self.completion.take()?;
        results.copy_from(&self.results);
        self.results.clear();
        self.active = None;
        Some(completion)
    }

    pub fn cancel(&mut self, request: ScanRequest) -> bool {
        if self.active != Some(request) {
            return false;
        }
        self.active = None;
        self.completion = None;
        self.results.clear();
        true
    }
}
