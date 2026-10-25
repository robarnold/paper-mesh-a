//! Read an idle diagnostic frame without changing display controls or image content.

pub mod protocol;

use alloc::{
    boxed::Box,
    string::{String, ToString},
};
use core::sync::atomic::{AtomicU32, Ordering};
use core::{future::Future, pin::Pin};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::Instant;
use serde_json::json;

use crate::{display_recovery, display_worker::Client, usb, usb_protocol};
use protocol::{Command, Request};

pub struct Pending {
    request: Request,
    owner: u32,
    hold_id: u32,
}

static REQUESTS: Channel<CriticalSectionRawMutex, Pending, 1> = Channel::new();
static NEXT_HOLD: AtomicU32 = AtomicU32::new(0);
static ACTIVE_HOLD: AtomicU32 = AtomicU32::new(0);

pub struct Hold {
    id: u32,
    stage: &'static str,
}

impl Hold {
    pub fn new(stage: &'static str) -> Self {
        let id = NEXT_HOLD.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
        assert_ne!(id, 0);
        assert_eq!(ACTIVE_HOLD.swap(id, Ordering::AcqRel), 0);
        Self { id, stage }
    }

    pub fn serve<'a>(
        &'a self,
        display: Option<&'a Client>,
        pending: Pending,
    ) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
        Box::pin(async move {
            let owner = pending.owner;
            if let Some(reply) = self.reply(display, pending) {
                // The application retains frame ownership until the bounded reply finishes.
                let _ = usb::send_control_reply(owner, reply).await;
            }
        })
    }

    fn reply(&self, display: Option<&Client>, pending: Pending) -> Option<String> {
        if !usb::control_connected(pending.owner) {
            return None;
        }
        let id = pending.request.id;
        let reply = if pending.hold_id != self.id
            || ACTIVE_HOLD.load(Ordering::Acquire) != self.id
            || display_recovery::blocked()
        {
            usb_protocol::error(id, -32000, "Display hold changed or recovery is active")
        } else if let Some(display) = display {
            let result = match pending.request.command {
                Command::Inspect => display.inspect_frame().map(|framebuffer| {
                    json!({
                        "protocol": 1,
                        "stage": self.stage,
                        "frame_id": self.id,
                        "captured_at_ms": Instant::now().as_millis(),
                        "recovery_blocked": false,
                        "framebuffer": framebuffer
                    })
                }),
                Command::Framebuffer {
                    frame_id,
                    offset,
                    length,
                } => {
                    if frame_id != u64::from(self.id) {
                        return Some(usb_protocol::error(id, -32001, "Frame changed"));
                    }
                    display.read_frame_chunk(offset, length).map(
                        |hex| json!({"frame_id":self.id,"offset":offset,"length":length,"hex":hex}),
                    )
                }
            };
            match result {
                Ok(value) => {
                    let mut reply = json!({"jsonrpc":"2.0","id":id,"result":value}).to_string();
                    reply.push('\n');
                    reply
                }
                Err(error) => usb_protocol::error(id, -32000, &error.to_string()),
            }
        } else {
            usb_protocol::error(id, -32000, "Display unavailable")
        };
        Some(reply)
    }
}

pub fn reject(pending: Pending) -> Pin<Box<dyn Future<Output = ()>>> {
    Box::pin(async move {
        if usb::control_connected(pending.owner) {
            let reply = usb_protocol::error(pending.request.id, -32000, "Display hold ended");
            let _ = usb::send_control_reply(pending.owner, reply).await;
        }
    })
}

pub async fn request() -> Pending {
    REQUESTS.receive().await
}

impl Drop for Hold {
    fn drop(&mut self) {
        ACTIVE_HOLD.store(0, Ordering::Release);
        // A request queued at the same time as BOOT must not reach the next image.
        while REQUESTS.try_receive().is_ok() {}
    }
}

pub fn handle(bytes: &[u8], owner: u32) -> Option<String> {
    let request = match protocol::parse(bytes) {
        Ok(Some(request)) => request,
        Ok(None) => return None,
        Err(reply) => return Some(reply),
    };
    let hold_id = ACTIVE_HOLD.load(Ordering::Acquire);
    if hold_id == 0 || display_recovery::blocked() || !usb::control_connected(owner) {
        return Some(usb_protocol::error(
            request.id,
            -32000,
            "Inspection requires an idle display hold",
        ));
    }
    // Dequeue releases capacity before the response reaches the host. The next
    // request can then wait while the previous response completes its local ACK.
    match REQUESTS.try_send(Pending {
        request,
        owner,
        hold_id,
    }) {
        Ok(()) => None,
        Err(embassy_sync::channel::TrySendError::Full(pending)) => Some(usb_protocol::error(
            pending.request.id,
            -32000,
            "Inspection queue is full",
        )),
    }
}
