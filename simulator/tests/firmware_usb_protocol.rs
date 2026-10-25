extern crate alloc;

#[allow(dead_code)]
#[path = "../../firmware/src/usb_protocol.rs"]
mod protocol;

use protocol::{Framer, Route};

#[test]
fn fragmented_requests_keep_their_json_boundary() {
    let mut frame = Framer::default();
    let request = b"{\"jsonrpc\":\"2.0\",\"id\":17,\"method\":\"display.inspect\"}\r\n";
    let mut messages = Vec::new();
    for packet in request.chunks(7) {
        for byte in packet {
            if let Some(message) = frame.push(*byte) {
                messages.push(message.unwrap());
            }
        }
    }
    assert_eq!(messages.len(), 1);
    assert!(matches!(protocol::route(&messages[0]), Ok(Route::Diagnostic(id)) if id == 17));
}

#[test]
fn overflow_and_disconnect_do_not_join_requests() {
    let mut frame = Framer::default();
    for _ in 0..513 {
        assert!(frame.push(b'x').is_none());
    }
    assert!(frame.push(b'\n').unwrap().is_err());
    for byte in b"old partial" {
        frame.push(*byte);
    }
    frame.clear();
    let request = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"system.bootloader\"}";
    for byte in request {
        assert!(frame.push(*byte).is_none());
    }
    let message = frame.push(b'\n').unwrap().unwrap();
    assert_eq!(message.as_slice(), request);
    assert!(matches!(protocol::route(&message), Ok(Route::Bootloader(id)) if id == 1));
}
