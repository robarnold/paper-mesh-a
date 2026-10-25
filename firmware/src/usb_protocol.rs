//! Bounded CDC frames and routing for acknowledged device commands.

use alloc::string::{String, ToString};
use serde_json::{Value, json};

pub const MAX_REQUEST: usize = 512;

#[derive(Debug, PartialEq)]
pub enum Route {
    Ignore,
    Bootloader(Value),
    Diagnostic(Value),
}

#[derive(Default)]
pub struct Framer {
    bytes: heapless::Vec<u8, MAX_REQUEST>,
    overflow: bool,
}

impl Framer {
    pub fn clear(&mut self) {
        self.bytes.clear();
        self.overflow = false;
    }

    pub fn push(&mut self, byte: u8) -> Option<Result<heapless::Vec<u8, MAX_REQUEST>, ()>> {
        if byte == b'\n' {
            let result = if self.overflow {
                Err(())
            } else {
                Ok(core::mem::take(&mut self.bytes))
            };
            self.clear();
            return Some(result);
        }
        if !self.overflow && self.bytes.push(byte).is_err() {
            self.overflow = true;
        }
        None
    }
}

pub fn error(id: Value, code: i32, message: &str) -> String {
    let mut line =
        json!({"jsonrpc":"2.0", "id":id, "error":{"code":code,"message":message}}).to_string();
    line.push('\n');
    line
}

pub fn route(bytes: &[u8]) -> Result<Route, String> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| error(Value::Null, -32700, "Parse error"))?;
    let invalid = || error(Value::Null, -32600, "Invalid request");
    let object = value.as_object().ok_or_else(invalid)?;
    if value.get("jsonrpc").and_then(Value::as_str) != Some("2.0") || !value["method"].is_string() {
        return Err(invalid());
    }
    let Some(id) = object.get("id") else {
        return Ok(Route::Ignore);
    };
    if !(id.is_null()
        || id.as_i64().is_some()
        || id.as_u64().is_some()
        || id.as_str().is_some_and(|s| s.len() <= 64))
    {
        return Err(invalid());
    }
    if value["method"] != "system.bootloader" {
        return Ok(Route::Diagnostic(id.clone()));
    }
    // A reset requires a non-null correlation ID and has no parameters.
    if id.is_null() {
        return Err(invalid());
    }
    if let Some(params) = object.get("params")
        && !params.as_object().is_some_and(|p| p.is_empty())
    {
        return Err(error(id.clone(), -32602, "Invalid params"));
    }
    Ok(Route::Bootloader(id.clone()))
}
