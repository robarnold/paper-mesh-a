extern crate alloc;

#[path = "../../firmware/src/usb_protocol.rs"]
mod protocol;
#[path = "../../firmware/src/device_control/safety.rs"]
mod safety;

use protocol::{Framer, Route, route};
use safety::Check;
use serde_json::{Value, json};

#[test]
fn only_explicit_correlated_requests_enter_the_downloader() {
    for id in [json!(1), json!("flash-next")] {
        for params in [None, Some(json!({}))] {
            let mut request = json!({"jsonrpc":"2.0", "id":id, "method":"system.bootloader"});
            if let Some(params) = params {
                request["params"] = params;
            }
            assert_eq!(
                route(request.to_string().as_bytes()),
                Ok(Route::Bootloader(id.clone()))
            );
        }
    }
    assert_eq!(
        route(br#"{"jsonrpc":"2.0","method":"system.bootloader"}"#),
        Ok(Route::Ignore)
    );
    for id in [
        Value::Null,
        json!(false),
        json!([]),
        json!({}),
        json!("x".repeat(65)),
    ] {
        let request = json!({"jsonrpc":"2.0", "id":id, "method":"system.bootloader"});
        assert!(route(request.to_string().as_bytes()).is_err());
    }
    for params in [json!({"force":true}), Value::Null, json!([]), json!(true)] {
        let request =
            json!({"jsonrpc":"2.0", "id":7, "method":"system.bootloader", "params":params});
        let reply: Value =
            serde_json::from_str(&route(request.to_string().as_bytes()).unwrap_err()).unwrap();
        assert_eq!(reply["id"], 7);
        assert_eq!(reply["error"]["code"], -32602);
    }
    for bytes in [
        b"[]".as_slice(),
        b"{",
        br#"{"id":1,"method":"system.bootloader"}"#,
    ] {
        assert!(route(bytes).is_err());
    }
    assert_eq!(
        route(br#"{"jsonrpc":"2.0","id":2,"method":"display.test.status"}"#),
        Ok(Route::Diagnostic(json!(2)))
    );
}

#[test]
fn fragment_overflow_and_disconnect_cannot_create_a_reset_request() {
    let mut frame = Framer::default();
    let request = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"system.bootloader\"}\n";
    for byte in &request[..request.len() - 1] {
        assert!(frame.push(*byte).is_none());
    }
    assert!(matches!(
        route(&frame.push(b'\n').unwrap().unwrap()),
        Ok(Route::Bootloader(_))
    ));
    for _ in 0..513 {
        frame.push(b'x');
    }
    assert!(frame.push(b'\n').unwrap().is_err());
    for byte in &request[..20] {
        frame.push(*byte);
    }
    frame.clear();
    for byte in &request[20..request.len() - 1] {
        frame.push(*byte);
    }
    assert!(route(&frame.push(b'\n').unwrap().unwrap()).is_err());
}

fn idle() -> Check {
    Check {
        connected: true,
        bus_available: true,
        display_in_flight: false,
        display_blocked: false,
        power_requested: false,
        watchdog_idle: true,
    }
}

#[test]
fn each_unverified_or_busy_condition_prevents_reset() {
    assert_eq!(idle().validate(), Ok(()));
    for index in 0..6 {
        let mut check = idle();
        match index {
            0 => check.connected = false,
            1 => check.bus_available = false,
            2 => check.display_in_flight = true,
            3 => check.display_blocked = true,
            4 => check.power_requested = true,
            _ => check.watchdog_idle = false,
        }
        assert!(check.validate().is_err(), "condition {index}");
    }
}
