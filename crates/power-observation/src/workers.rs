//! Task storage for a prepared power observer.
//!
//! # Design choices
//!
//! - Box the future once so device types do not require task declarations in firmware.
//! - Let the runtime choose the spawner through the platform worker contract.
//!
//! See [power observation ownership](../../../docs/adapters/power-observation.md).

use alloc::boxed::Box;
use core::{future::Future, pin::Pin};
use embassy_executor::{SpawnError, Spawner};
use paper_mesh_platform::worker::WorkerSet;

/// Prepared observation task with device resources retained until task completion.
#[must_use]
pub struct Workers {
    pub(super) future: Pin<Box<dyn Future<Output = ()> + 'static>>,
}

impl WorkerSet for Workers {
    fn spawn(self, spawner: Spawner) -> Result<(), SpawnError> {
        spawner.spawn(worker(self)?);
        Ok(())
    }
}

#[embassy_executor::task]
async fn worker(workers: Workers) {
    workers.future.await;
}
