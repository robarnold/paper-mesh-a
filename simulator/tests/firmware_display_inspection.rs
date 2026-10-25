extern crate alloc;

#[path = "../../firmware/src/display_inspection/protocol.rs"]
mod protocol;
#[allow(dead_code)]
#[path = "../../firmware/src/usb_protocol.rs"]
mod usb_protocol;

use paper_mesh_ed047tc1::FRAME_BYTES;
use protocol::{Command, MAX_CHUNK_BYTES, parse};
use serde_json::{Value, json};

fn request(method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0", "id":"read-1", "method":method, "params":params})
}

fn framebuffer(frame_id: Value, offset: Value, length: Value) -> Value {
    request(
        "display.framebuffer.read",
        json!({"frame_id":frame_id, "offset":offset, "length":length}),
    )
}

fn error(bytes: &[u8]) -> Value {
    let line = parse(bytes).unwrap_err();
    assert!(line.ends_with('\n'));
    serde_json::from_str(&line).unwrap()
}

fn invalid_params(value: Value) {
    let reply = error(value.to_string().as_bytes());
    assert_eq!(reply["jsonrpc"], "2.0");
    assert_eq!(reply["id"], "read-1");
    assert_eq!(reply["error"]["code"], -32602);
}

#[test]
fn inspect_accepts_only_absent_or_empty_object_params() {
    for params in [None, Some(json!({}))] {
        let mut value = json!({"jsonrpc":"2.0", "id":1, "method":"display.inspect"});
        if let Some(params) = params {
            value["params"] = params;
        }
        let parsed = parse(value.to_string().as_bytes()).unwrap().unwrap();
        assert_eq!(parsed.id, 1);
        assert_eq!(parsed.command, Command::Inspect);
    }
    for params in [
        Value::Null,
        json!([]),
        json!(false),
        json!(1),
        json!({"refresh":true}),
    ] {
        invalid_params(request("display.inspect", params));
    }
}

#[test]
fn reads_require_a_non_null_supported_correlation_id() {
    for id in [
        json!(i64::MIN),
        json!(0),
        json!(u64::MAX),
        json!(""),
        json!("x".repeat(64)),
    ] {
        let mut value = request("display.inspect", json!({}));
        value["id"] = id.clone();
        assert_eq!(parse(value.to_string().as_bytes()).unwrap().unwrap().id, id);
    }
    for id in [
        Value::Null,
        json!(false),
        json!(1.5),
        json!(1.0),
        json!([]),
        json!({}),
        json!("x".repeat(65)),
        json!("é".repeat(33)),
    ] {
        let mut value = request("display.inspect", json!({}));
        value["id"] = id;
        let reply = error(value.to_string().as_bytes());
        assert_eq!(reply["id"], Value::Null);
        assert_eq!(reply["error"]["code"], -32600);
    }
}

#[test]
fn notifications_never_request_a_read() {
    for method in [
        "display.inspect",
        "display.framebuffer.read",
        "display.drive.read",
        "unknown",
    ] {
        let value = json!({"jsonrpc":"2.0", "method":method, "params":{"invalid":true}});
        assert_eq!(parse(value.to_string().as_bytes()), Ok(None));
    }
}

#[test]
fn malformed_json_and_invalid_envelopes_are_distinct() {
    for bytes in [b"{".as_slice(), b"", b"\xff", b"{} trailing"] {
        assert_eq!(error(bytes)["error"]["code"], -32700);
    }
    for value in [
        Value::Null,
        json!([]),
        json!(1),
        json!({}),
        json!({"jsonrpc":"1.0","id":1,"method":"display.inspect"}),
        json!({"jsonrpc":"2.0","id":1,"method":7}),
        json!({"jsonrpc":"2.0","id":1,"method":"display.inspect","extra":true}),
    ] {
        let reply = error(value.to_string().as_bytes());
        assert_eq!(reply["id"], Value::Null);
        assert_eq!(reply["error"]["code"], -32600);
    }
}

#[test]
fn unknown_methods_return_a_correlated_error() {
    let reply = error(request("display.refresh", json!({})).to_string().as_bytes());
    assert_eq!(reply["id"], "read-1");
    assert_eq!(reply["error"]["code"], -32601);
}

#[test]
fn framebuffer_accepts_first_last_and_maximum_chunks() {
    assert_eq!(FRAME_BYTES, 259_200);
    for (frame_id, offset, length) in [
        (1, 0, 1),
        (u64::MAX, 0, MAX_CHUNK_BYTES),
        (2, FRAME_BYTES - MAX_CHUNK_BYTES, MAX_CHUNK_BYTES),
        (3, FRAME_BYTES - 1, 1),
    ] {
        let value = framebuffer(json!(frame_id), json!(offset), json!(length));
        assert_eq!(
            parse(value.to_string().as_bytes())
                .unwrap()
                .unwrap()
                .command,
            Command::Framebuffer {
                frame_id,
                offset,
                length
            }
        );
    }
}

#[test]
fn captured_reads_reject_missing_extra_or_non_object_params() {
    for params in [
        Value::Null,
        json!([]),
        json!("frame"),
        json!({}),
        json!({"frame_id":1,"offset":0}),
        json!({"frame_id":1,"length":1}),
        json!({"offset":0,"length":1}),
        json!({"frame_id":1,"offset":0,"size":1}),
        json!({"frame_id":1,"offset":0,"length":1,"refresh":true}),
    ] {
        invalid_params(request("display.framebuffer.read", params));
    }
    let mut value = request("display.framebuffer.read", json!({}));
    value.as_object_mut().unwrap().remove("params");
    invalid_params(value);
}

#[test]
fn captured_reads_reject_non_integral_and_invalid_numeric_fields() {
    for invalid in [
        Value::Null,
        json!(false),
        json!("1"),
        json!(-1),
        json!(1.0),
        json!(1.5),
        json!([]),
        json!({}),
    ] {
        invalid_params(framebuffer(invalid.clone(), json!(0), json!(1)));
        invalid_params(framebuffer(json!(1), invalid.clone(), json!(1)));
        invalid_params(framebuffer(json!(1), json!(0), invalid));
    }
    invalid_params(framebuffer(json!(0), json!(0), json!(1)));
}

#[test]
fn framebuffer_rejects_empty_oversized_crossing_and_overflowing_chunks() {
    for (offset, length) in [
        (0, 0),
        (0, 513),
        (FRAME_BYTES as u64, 1),
        (FRAME_BYTES as u64 - 1, 2),
        (FRAME_BYTES as u64 - 511, 512),
        (u64::MAX, 1),
        (u64::MAX - 511, 512),
        (0, u64::MAX),
    ] {
        invalid_params(framebuffer(json!(1), json!(offset), json!(length)));
    }
    // An out-of-range JSON number must not round into an accepted frame ID.
    let bytes = br#"{"jsonrpc":"2.0","id":"read-1","method":"display.framebuffer.read","params":{"frame_id":18446744073709551616,"offset":0,"length":1}}"#;
    assert_eq!(error(bytes)["error"]["code"], -32602);
}

#[test]
fn removed_experiment_commands_are_unknown() {
    for method in [
        "display.drive.read",
        "display.test.start",
        "display.test.abort",
        "display.test.status",
    ] {
        let reply = error(request(method, json!({})).to_string().as_bytes());
        assert_eq!(reply["id"], "read-1");
        assert_eq!(reply["error"]["code"], -32601);
    }
}
