//! Parse acknowledged requests for display state and bounded reads of captured data.

use crate::usb_protocol::error;
use alloc::string::String;
use paper_mesh_ed047tc1::FRAME_BYTES;
use serde_json::Value;

pub const MAX_CHUNK_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Inspect,
    Framebuffer {
        frame_id: u64,
        offset: usize,
        length: usize,
    },
}

#[derive(Debug, PartialEq)]
pub struct Request {
    pub id: Value,
    pub command: Command,
}

pub fn parse(bytes: &[u8]) -> Result<Option<Request>, String> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| error(Value::Null, -32700, "Parse error"))?;
    let invalid = || error(Value::Null, -32600, "Invalid request");
    let object = value.as_object().ok_or_else(invalid)?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "jsonrpc" | "id" | "method" | "params"))
        || value.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || !value["method"].is_string()
    {
        return Err(invalid());
    }
    // A notification does not read the display or send a response.
    let Some(id) = object.get("id") else {
        return Ok(None);
    };
    if !(id.as_i64().is_some()
        || id.as_u64().is_some()
        || id.as_str().is_some_and(|text| text.len() <= 64))
    {
        return Err(invalid());
    }
    let bad_params = || error(id.clone(), -32602, "Invalid params");
    let command = match value["method"].as_str().unwrap() {
        "display.inspect" => {
            if let Some(params) = object.get("params")
                && !params.as_object().is_some_and(|params| params.is_empty())
            {
                return Err(bad_params());
            }
            Command::Inspect
        }
        "display.framebuffer.read" => {
            let params = value["params"].as_object().ok_or_else(bad_params)?;
            if params.len() != 3 {
                return Err(bad_params());
            }
            let frame_id = params
                .get("frame_id")
                .and_then(Value::as_u64)
                .filter(|value| *value > 0)
                .ok_or_else(bad_params)?;
            let offset = params
                .get("offset")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(bad_params)?;
            let length = params
                .get("length")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| (1..=MAX_CHUNK_BYTES).contains(value))
                .ok_or_else(bad_params)?;
            if !offset
                .checked_add(length)
                .is_some_and(|end| end <= FRAME_BYTES)
            {
                return Err(bad_params());
            }
            Command::Framebuffer {
                frame_id,
                offset,
                length,
            }
        }
        _ => return Err(error(id.clone(), -32601, "Method not found")),
    };
    Ok(Some(Request {
        id: id.clone(),
        command,
    }))
}
