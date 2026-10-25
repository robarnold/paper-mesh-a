//! HTTP transport for the native UI guide.

use crate::{HttpResponse, response};
use paper_mesh_simulator::ui_guide::{self, Guide, Sheet};
use std::io::Read;
use tiny_http::{Method, Request};

pub fn handle(request: &mut Request, guide: &mut Guide) -> HttpResponse {
    let path = request.url().split('?').next().unwrap_or("");
    match (request.method(), path) {
        (&Method::Get, "/ui-guide" | "/ui-guide/") => response(
            200,
            "text/html; charset=utf-8",
            include_bytes!("ui_guide/index.html").to_vec(),
        ),
        (&Method::Get, "/ui-guide/state") => state(guide),
        (&Method::Get, "/ui-guide/controls/state") => controls_state(guide),
        (&Method::Get, "/ui-guide/text/state") => text_state(guide),
        (&Method::Get, "/ui-guide/snippets") => response(
            200,
            "application/json",
            serde_json::to_vec(&ui_guide::snippets()).unwrap(),
        ),
        (&Method::Get, "/ui-guide/source.rs") => {
            response(200, "text/plain; charset=utf-8", ui_guide::SOURCE)
        }
        (&Method::Get, "/ui-guide/catalog.rs") => {
            response(200, "text/plain; charset=utf-8", ui_guide::CATALOG_SOURCE)
        }
        (&Method::Get, "/ui-guide/controls.rs") => {
            response(200, "text/plain; charset=utf-8", ui_guide::CONTROLS_SOURCE)
        }
        (&Method::Get, "/ui-guide/text.rs") => {
            response(200, "text/plain; charset=utf-8", ui_guide::TEXT_SOURCE)
        }
        (&Method::Get, "/ui-guide/reference/shared") => response(
            200,
            "text/plain; charset=utf-8",
            include_str!("../../docs/ui.md"),
        ),
        (&Method::Get, "/ui-guide/reference/application") => response(
            200,
            "text/plain; charset=utf-8",
            include_str!("../../docs/application-ui.md"),
        ),
        (&Method::Get, "/ui-guide/reference/toolkit") => response(
            200,
            "text/plain; charset=utf-8",
            include_str!("../../docs/platform/toolkit.md"),
        ),
        (
            &Method::Get,
            path @ ("/ui-guide/frame.png"
            | "/ui-guide/catalog.png"
            | "/ui-guide/text.png"
            | "/ui-guide/controls.png"),
        ) => {
            let png = match path {
                "/ui-guide/frame.png" => guide.png(),
                "/ui-guide/text.png" => guide.text_png(),
                "/ui-guide/controls.png" => guide.controls_png(),
                _ => ui_guide::catalog_png(Sheet::Overview),
            };
            match png {
                Ok(bytes) => response(200, "image/png", bytes),
                Err(error) => {
                    eprintln!("Guide render failed: {error}");
                    response(500, "text/plain", "Guide render failed.")
                }
            }
        }
        (
            &Method::Post,
            path @ ("/ui-guide/event" | "/ui-guide/controls/event" | "/ui-guide/text/event"),
        ) => {
            let controls = path == "/ui-guide/controls/event";
            let text = path == "/ui-guide/text/event";
            if !request.headers().iter().any(|header| {
                header.field.equiv("Content-Type") && header.value.as_str() == "application/json"
            }) {
                return response(415, "text/plain", "Expected application/json.");
            }
            let mut body = Vec::new();
            if request
                .as_reader()
                .take(1025)
                .read_to_end(&mut body)
                .is_err()
                || body.len() > 1024
            {
                return response(400, "text/plain", "Invalid or oversized event.");
            }
            match serde_json::from_slice::<ui_guide::Command>(&body) {
                Ok(command) => match if controls {
                    guide.controls_command(command)
                } else if text {
                    guide.text_command(command)
                } else {
                    guide.command(command)
                } {
                    Ok(()) if controls => controls_state(guide),
                    Ok(()) if text => text_state(guide),
                    Ok(()) => state(guide),
                    Err(message) => response(400, "text/plain", message),
                },
                Err(_) => response(400, "text/plain", "Invalid guide event."),
            }
        }
        _ => response(404, "text/plain", "Not found."),
    }
}

fn state(guide: &Guide) -> HttpResponse {
    response(
        200,
        "application/json",
        serde_json::to_vec(&guide.state()).unwrap(),
    )
}

fn controls_state(guide: &Guide) -> HttpResponse {
    response(
        200,
        "application/json",
        serde_json::to_vec(&guide.controls_state()).unwrap(),
    )
}

fn text_state(guide: &Guide) -> HttpResponse {
    response(
        200,
        "application/json",
        serde_json::to_vec(&guide.text_state()).unwrap(),
    )
}
