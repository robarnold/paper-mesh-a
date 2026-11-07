//! HTTP transport for the native UI guide.
//!
//! # Design choices
//!
//! - Route each guide request to its own example so commands and preview frames use
//!   the same state.
//!
//! See [UI guide](../../docs/simulator/api.md#ui-guide).

use crate::{HttpRequest, HttpResponse, response};
use axum::http::{Method, header};
use paper_mesh_simulator::ui_guide::{self, Guide, Sheet};

fn guide_build() -> String {
    std::env::var("PAPER_MESH_GUIDE_BUILD")
        .ok()
        .and_then(|value| value.parse::<u128>().ok())
        .map(|id| id.to_string())
        .unwrap_or_default()
}

pub fn handle(request: &HttpRequest, guide: &mut Guide) -> HttpResponse {
    let path = request.uri().path().split('?').next().unwrap_or("");
    match (request.method(), path) {
        (&Method::GET, "/ui-guide" | "/ui-guide/") => response(
            200,
            "text/html; charset=utf-8",
            include_str!("ui_guide/index.html").replace("{{guide_build}}", &guide_build()),
        ),
        (&Method::GET, "/ui-guide/build") => response(200, "text/plain", guide_build()),
        (&Method::GET, "/ui-guide/keyboard/state") => keyboard_state(guide),
        (&Method::GET, "/ui-guide/state") => state(guide),
        (&Method::GET, "/ui-guide/controls/state") => controls_state(guide),
        (&Method::GET, "/ui-guide/text/state") => text_state(guide),
        (&Method::GET, "/ui-guide/calculator/state") => calculator_state(guide),
        (&Method::GET, "/ui-guide/calculator.rs") => response(
            200,
            "text/plain; charset=utf-8",
            ui_guide::CALCULATOR_SOURCE,
        ),
        (&Method::GET, "/ui-guide/calculator.png") => match guide.calculator_png() {
            Ok(bytes) => response(200, "image/png", bytes),
            Err(_) => response(500, "text/plain", "Guide render failed."),
        },
        (&Method::GET, "/ui-guide/snippets") => response(
            200,
            "application/json",
            serde_json::to_vec(&ui_guide::snippets()).unwrap(),
        ),
        (&Method::GET, "/ui-guide/source.rs") => {
            response(200, "text/plain; charset=utf-8", ui_guide::SOURCE)
        }
        (&Method::GET, "/ui-guide/catalog.rs") => {
            response(200, "text/plain; charset=utf-8", ui_guide::CATALOG_SOURCE)
        }
        (&Method::GET, "/ui-guide/controls.rs") => {
            response(200, "text/plain; charset=utf-8", ui_guide::CONTROLS_SOURCE)
        }
        (&Method::GET, "/ui-guide/text.rs") => {
            response(200, "text/plain; charset=utf-8", ui_guide::TEXT_SOURCE)
        }
        (&Method::GET, "/ui-guide/reference/shared") => response(
            200,
            "text/plain; charset=utf-8",
            include_str!("../../docs/ui.md"),
        ),
        (&Method::GET, "/ui-guide/reference/application") => response(
            200,
            "text/plain; charset=utf-8",
            include_str!("../../docs/application-ui.md"),
        ),
        (&Method::GET, "/ui-guide/reference/toolkit") => response(
            200,
            "text/plain; charset=utf-8",
            include_str!("../../docs/platform/toolkit.md"),
        ),
        (
            &Method::GET,
            path @ ("/ui-guide/frame.png"
            | "/ui-guide/keyboard.png"
            | "/ui-guide/catalog.png"
            | "/ui-guide/text.png"
            | "/ui-guide/controls.png"),
        ) => {
            let png = match path {
                "/ui-guide/keyboard.png" => guide.keyboard_png(),
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
            &Method::POST,
            path @ ("/ui-guide/event"
            | "/ui-guide/keyboard/event"
            | "/ui-guide/controls/event"
            | "/ui-guide/calculator/event"
            | "/ui-guide/text/event"),
        ) => {
            let keyboard = path == "/ui-guide/keyboard/event";
            let controls = path == "/ui-guide/controls/event";
            let text = path == "/ui-guide/text/event";
            let calculator = path == "/ui-guide/calculator/event";
            if request
                .headers()
                .get(header::CONTENT_TYPE)
                .is_none_or(|value| value != "application/json")
            {
                return response(415, "text/plain", "Expected application/json.");
            }
            let body = request.body();
            match serde_json::from_slice::<ui_guide::Command>(body) {
                Ok(command) => match if calculator {
                    guide.calculator_command(command)
                } else if keyboard {
                    guide.keyboard_command(command)
                } else if controls {
                    guide.controls_command(command)
                } else if text {
                    guide.text_command(command)
                } else {
                    guide.command(command)
                } {
                    Ok(()) if calculator => calculator_state(guide),
                    Ok(()) if keyboard => keyboard_state(guide),
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

fn keyboard_state(guide: &Guide) -> HttpResponse {
    response(200, "application/json", guide.keyboard_state().to_string())
}

fn calculator_state(guide: &Guide) -> HttpResponse {
    response(
        200,
        "application/json",
        serde_json::to_vec(&guide.calculator_state()).unwrap(),
    )
}
