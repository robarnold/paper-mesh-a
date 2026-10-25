//! Enter the ROM downloader only while the retained application future is paused.

mod rom;
mod safety;

use alloc::string::{String, ToString};
use core::sync::atomic::{AtomicBool, Ordering};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, with_timeout};
use serde_json::{Value, json};

use crate::{
    bus::Bus, display, display_recovery, display_worker, recovery, suspend, usb, usb_protocol,
};

pub struct Request {
    id: Value,
    owner: u32,
}

static REQUESTS: Channel<CriticalSectionRawMutex, Request, 1> = Channel::new();
static PENDING: AtomicBool = AtomicBool::new(false);

pub fn queue(id: Value, owner: u32) -> Option<String> {
    if PENDING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Some(usb_protocol::error(
            id,
            -32000,
            "Download request already pending",
        ));
    }
    // Only the caller that acquired PENDING can insert a request.
    assert!(REQUESTS.try_send(Request { id, owner }).is_ok());
    None
}

pub async fn request() -> Request {
    REQUESTS.receive().await
}

fn check(bus: Option<&Bus>, owner: u32) -> Result<(), &'static str> {
    safety::Check {
        connected: usb::control_connected(owner),
        bus_available: bus.is_some(),
        display_in_flight: display_worker::in_flight(),
        display_blocked: display_recovery::blocked(),
        power_requested: suspend::requested(),
        watchdog_idle: recovery::idle(),
    }
    .validate()
}

/// The caller must retain, but not poll, the sole application future until this returns.
pub async fn execute(request: Request, bus: Option<&Bus>) {
    let result = prepare(bus, request.owner).await;
    match result {
        Ok(_parked) => {
            let mut reply = json!({"jsonrpc":"2.0", "id":request.id,
                "result":{"accepted":true,"mode":"rom-download"}})
            .to_string();
            reply.push('\n');
            if usb::send_control_reply(request.owner, reply).await
                && check(bus, request.owner).is_ok()
            {
                // No application poll or display submission can occur between this check and reset.
                rom::enter();
            }
            log::warn!("download: acknowledgement lost; application resumes");
        }
        Err(reason) => {
            let reply = usb_protocol::error(request.id, -32000, reason);
            let _ = usb::send_control_reply(request.owner, reply).await;
            log::info!("download: refused; {}", reason);
        }
    }
    PENDING.store(false, Ordering::Release);
}

struct Parked;

impl Drop for Parked {
    fn drop(&mut self) {
        suspend::cores::release();
    }
}

async fn prepare(bus: Option<&Bus>, owner: u32) -> Result<Parked, &'static str> {
    check(bus, owner)?;
    if !rom::supported() {
        return Err("ROM USB download is unavailable on this chip configuration");
    }
    let bus = bus.ok_or("Power controls unavailable")?;
    match with_timeout(Duration::from_secs(1), suspend::cores::request_park()).await {
        Ok(Ok(())) => {}
        Ok(Err(_)) => return Err("Display core is busy"),
        Err(_) => {
            suspend::cores::release();
            return Err("Display core did not stop");
        }
    }
    let parked = Parked;
    // These reads preserve the existing idle state and do not touch the PMIC or its EEPROM.
    display::verify_controls_off(bus).map_err(|_| "Display controls are not confirmed off")?;
    check(Some(bus), owner)?;
    Ok(parked)
}
