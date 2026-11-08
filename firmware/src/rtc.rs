//! RTC task entry point for the firmware executor.
//!
//! # Design choices
//!
//! - Keep the concrete task in the runtime so the adapter does not select an executor or bus queue capacity.
//!
//! See [ESP clock integration](../../docs/adapters/rtc.md#esp-clock-integration).

use paper_mesh_esp_adapters::rtc::ClockWorker;
use paper_mesh_platform::location::LocationSnapshot;

use crate::bus::Bus;

#[embassy_executor::task]
pub async fn worker(bus: &'static Bus, location: fn(u64) -> LocationSnapshot, worker: ClockWorker) {
    worker.run(bus, location).await;
}
