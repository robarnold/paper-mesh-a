//! HTTP host and wall-clock pacing for the device simulator.
//!
//! # Design choices
//!
//! - Translate HTTP input into simulation commands and pace the virtual clock so
//!   callers can pause it for reproducible observations.
//!
//! See [Simulator API](../../docs/simulator/api.md).

use std::{
    env, fs,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::{Bytes, to_bytes},
    extract::{Request, State as HttpState},
    http::{Method, header},
    response::Response,
};
use embedded_graphics::prelude::*;
use paper_mesh_platform::{TouchEvent, touchpad::HostState};
use paper_mesh_simulator::SCREEN_SIZE;
use paper_mesh_simulator::discovery::ScanScenario;
use paper_mesh_simulator::location::LocationScenario;
use paper_mesh_simulator::power::PowerScenario;
use paper_mesh_simulator::{REFRESH_MS, Simulation, Snapshot};
use serde::{Deserialize, Serialize};

type Error = Box<dyn std::error::Error + Send + Sync>;
type HttpResponse = Response;
type HttpRequest = axum::http::Request<Bytes>;

mod ui_guide_http;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    TouchpadHost {
        state: HostScenario,
    },
    HardwareButton {
        id: u8,
        pressed: bool,
    },
    TouchDown {
        #[serde(default)]
        id: u8,
        x: i32,
        y: i32,
    },
    TouchMove {
        #[serde(default)]
        id: u8,
        x: i32,
        y: i32,
    },
    TouchUp {
        #[serde(default)]
        id: u8,
        x: i32,
        y: i32,
    },
    TouchCancel,
    Pause,
    Resume,
    Advance {
        ms: u64,
    },
    Reset,
    ScanScenario {
        scenario: ScanScenario,
    },
    LocationScenario {
        scenario: LocationScenario,
    },
    PowerScenario {
        scenario: PowerScenario,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum HostScenario {
    Ready,
    Disconnected,
    Suspended,
    Fault,
}

#[derive(Serialize)]
struct State {
    #[serde(flatten)]
    device: Snapshot,
    running: bool,
    generation: u64,
}

struct Session {
    simulation: Simulation,
    running: bool,
    generation: u64,
}

impl Session {
    fn state(&self) -> State {
        State {
            device: self.simulation.snapshot(),
            running: self.running,
            generation: self.generation,
        }
    }

    fn command(&mut self, command: Command) -> Result<(), &'static str> {
        match command {
            Command::TouchpadHost { state } => self.simulation.set_touchpad_host(match state {
                HostScenario::Ready => HostState::Ready,
                HostScenario::Disconnected => HostState::Disconnected,
                HostScenario::Suspended => HostState::Suspended,
                HostScenario::Fault => HostState::Fault,
            }),
            Command::HardwareButton { id, pressed } => {
                self.simulation.hardware_button(id, pressed)?
            }
            Command::TouchDown { id, x, y }
            | Command::TouchMove { id, x, y }
            | Command::TouchUp { id, x, y } => {
                if id > 15 {
                    return Err("Contact ID must be between 0 and 15.");
                }
                if x < 0 || y < 0 || x >= SCREEN_SIZE.width as i32 || y >= SCREEN_SIZE.height as i32
                {
                    return Err("Touch must be inside the display.");
                }
                let point = Point::new(x, y);
                let event = if !matches!(command, Command::TouchUp { .. }) {
                    TouchEvent::touch(id, point)
                } else {
                    TouchEvent::release(id, point)
                };
                self.simulation.touch_portrait(event);
            }
            Command::TouchCancel => self.simulation.touch_portrait(TouchEvent::Cancel),
            Command::Pause => self.running = false,
            Command::Resume => self.running = true,
            Command::Advance { ms } => {
                if self.running {
                    return Err("Pause before advancing virtual time.");
                }
                if !(1..=60_000).contains(&ms) {
                    return Err("Advance must be between 1 and 60000 milliseconds.");
                }
                self.simulation.advance(ms);
            }
            Command::Reset => {
                self.simulation = Simulation::default();
                self.generation += 1;
            }
            Command::ScanScenario { scenario } => self.simulation.set_scan_scenario(scenario),
            Command::LocationScenario { scenario } => {
                self.simulation.set_location_scenario(scenario)
            }
            Command::PowerScenario { scenario } => self.simulation.set_power_scenario(scenario),
        }
        Ok(())
    }
}

fn response(status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> HttpResponse {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(axum::body::Body::from(body.into()))
        .unwrap()
}

fn handle(
    request: &HttpRequest,
    session: &mut Session,
    guide: &mut paper_mesh_simulator::ui_guide::Guide,
) -> HttpResponse {
    let url = request
        .uri()
        .path_and_query()
        .map_or("/", |value| value.as_str());
    if url == "/ui-guide" || url.starts_with("/ui-guide/") {
        return ui_guide_http::handle(request, guide);
    }
    match (request.method(), url) {
        (&Method::GET, "/") => response(
            200,
            "text/html; charset=utf-8",
            include_bytes!("index.html").to_vec(),
        ),
        (&Method::GET, "/api/state") => response(
            200,
            "application/json",
            serde_json::to_vec(&session.state())
                .expect("state contains only JSON-compatible values"),
        ),
        (&Method::GET, path @ ("/frame.png" | "/frame-native.png")) => {
            match if path == "/frame.png" {
                session.simulation.portrait_png()
            } else {
                session.simulation.display().native_png()
            } {
                Ok(png) => response(200, "image/png", png),
                Err(error) => {
                    eprintln!("PNG encoding failed: {error}");
                    response(500, "text/plain", "Frame encoding failed.")
                }
            }
        }
        (&Method::POST, "/api/event") => {
            if request
                .headers()
                .get(header::CONTENT_TYPE)
                .is_none_or(|value| value != "application/json")
            {
                return response(415, "text/plain", "Expected application/json.");
            }
            let body = request.body();
            let command = match serde_json::from_slice::<Command>(body) {
                Ok(command) => command,
                Err(_) => return response(400, "text/plain", "Invalid event."),
            };
            match session.command(command) {
                Ok(()) => response(200, "text/plain", "OK"),
                Err(message) => response(400, "text/plain", message),
            }
        }
        _ => response(404, "text/plain", "Not found."),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().skip(1).collect();
    if let [flag, destination] = args.as_slice()
        && matches!(flag.as_str(), "--snapshot" | "--snapshot-native")
    {
        let mut sim = Simulation::default();
        sim.advance(REFRESH_MS);
        let path = Path::new(destination);
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        let png = if flag == "--snapshot-native" {
            sim.display().native_png()?
        } else {
            sim.portrait_png()?
        };
        fs::write(path, png)?;
        println!("Saved {}", path.display());
        return Ok(());
    }
    if !args.is_empty() {
        return Err("Usage: paper-mesh-simulator [--snapshot|--snapshot-native FILE.png]".into());
    }

    // Docker publishes this port only on the host loopback interface.
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    let host = Arc::new(Mutex::new(Host {
        session: Session {
            simulation: Simulation::default(),
            running: true,
            generation: 0,
        },
        guide: paper_mesh_simulator::ui_guide::Guide::default(),
        last_wall: Instant::now(),
        remainder: Duration::ZERO,
    }));
    let app = Router::new()
        .fallback(serve_request)
        .with_state(host.clone());
    println!("Paper Mesh simulator: http://localhost:8080");
    println!("The device application, rendering, and virtual hardware run in this Rust process.");
    let pacing = async {
        let mut timer = tokio::time::interval(Duration::from_millis(10));
        loop {
            timer.tick().await;
            host.lock().unwrap().advance();
        }
    };
    tokio::select! {
        result = axum::serve(listener, app) => result?,
        () = pacing => {},
    }
    Ok(())
}

struct Host {
    session: Session,
    guide: paper_mesh_simulator::ui_guide::Guide,
    last_wall: Instant,
    remainder: Duration,
}

impl Host {
    fn advance(&mut self) {
        let wall = Instant::now();
        if self.session.running {
            self.remainder += wall.duration_since(self.last_wall);
            let milliseconds = self.remainder.as_millis().min(u64::MAX as u128) as u64;
            self.remainder -= Duration::from_millis(milliseconds);
            self.session.simulation.advance(milliseconds);
        } else {
            self.remainder = Duration::ZERO;
        }
        self.last_wall = wall;
    }

    fn handle(&mut self, request: &HttpRequest) -> HttpResponse {
        self.advance();
        let was_running = self.session.running;
        let generation = self.session.generation;
        let result = handle(request, &mut self.session, &mut self.guide);
        if was_running != self.session.running || generation != self.session.generation {
            self.last_wall = Instant::now();
            self.remainder = Duration::ZERO;
        }
        result
    }
}

async fn serve_request(
    HttpState(host): HttpState<Arc<Mutex<Host>>>,
    request: Request,
) -> HttpResponse {
    let (parts, body) = request.into_parts();
    // Read network data before borrowing simulation state so a slow client cannot hold the lock.
    let body = match to_bytes(body, 1024).await {
        Ok(body) => body,
        Err(_) => return response(400, "text/plain", "Invalid or oversized event."),
    };
    host.lock()
        .unwrap()
        .handle(&HttpRequest::from_parts(parts, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    // HTTP commands must preserve paused time, frame responses, and event validation.
    #[tokio::test(flavor = "current_thread")]
    async fn http_preserves_commands_frames_and_limits() {
        let host = Arc::new(Mutex::new(Host {
            session: Session {
                simulation: Simulation::default(),
                running: false,
                generation: 0,
            },
            guide: paper_mesh_simulator::ui_guide::Guide::default(),
            last_wall: Instant::now(),
            remainder: Duration::ZERO,
        }));
        for (method, path, content_type, body, status) in [
            (
                "POST",
                "/api/event",
                "application/json",
                r#"{"type":"advance","ms":123}"#.to_owned(),
                200,
            ),
            ("POST", "/api/event", "text/plain", "{}".to_owned(), 415),
            (
                "POST",
                "/api/event",
                "application/json",
                "{".to_owned(),
                400,
            ),
            (
                "POST",
                "/api/event",
                "application/json",
                " ".repeat(1025),
                400,
            ),
            (
                "POST",
                "/ui-guide/event",
                "application/json",
                " ".repeat(1025),
                400,
            ),
            ("GET", "/missing", "", String::new(), 404),
            ("GET", "/ui-guide", "", String::new(), 200),
            ("GET", "/ui-guide/state", "", String::new(), 200),
        ] {
            let request = Request::builder()
                .method(method)
                .uri(path)
                .header(header::CONTENT_TYPE, content_type)
                .body(axum::body::Body::from(body))
                .unwrap();
            let result = serve_request(HttpState(host.clone()), request).await;
            assert_eq!(result.status().as_u16(), status, "{method} {path}");
            assert_eq!(result.headers()[header::CACHE_CONTROL], "no-store");
        }
        let request = Request::builder()
            .uri("/api/state")
            .body(axum::body::Body::empty())
            .unwrap();
        let result = serve_request(HttpState(host.clone()), request).await;
        let body = to_bytes(result.into_body(), usize::MAX).await.unwrap();
        let state: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(state["now_ms"], 123);
        assert_eq!(state["running"], false);
        for path in ["/frame.png", "/frame-native.png", "/ui-guide/keyboard.png"] {
            let request = Request::builder()
                .uri(path)
                .body(axum::body::Body::empty())
                .unwrap();
            let result = serve_request(HttpState(host.clone()), request).await;
            assert_eq!(result.status(), 200);
            assert_eq!(result.headers()[header::CONTENT_TYPE], "image/png");
            let body = to_bytes(result.into_body(), usize::MAX).await.unwrap();
            assert!(body.starts_with(b"\x89PNG\r\n\x1a\n"));
        }
    }

    #[test]
    fn stepping_requires_pause_and_rejects_invalid_time_or_coordinates() {
        let mut session = Session {
            simulation: Simulation::default(),
            running: true,
            generation: 0,
        };
        assert!(session.command(Command::Advance { ms: 1000 }).is_err());
        session.command(Command::Pause).unwrap();
        assert!(session.command(Command::Advance { ms: 60_001 }).is_err());
        assert!(
            session
                .command(Command::TouchDown {
                    id: 0,
                    x: 540,
                    y: 20
                })
                .is_err()
        );
        assert!(
            session
                .command(Command::TouchDown {
                    id: 0,
                    x: 20,
                    y: 960
                })
                .is_err()
        );
        assert!(
            session
                .command(Command::TouchUp {
                    id: 0,
                    x: -1,
                    y: 20
                })
                .is_err()
        );
        session
            .command(Command::TouchDown {
                id: 0,
                x: 539,
                y: 959,
            })
            .unwrap();
        session
            .command(Command::TouchUp {
                id: 0,
                x: 539,
                y: 959,
            })
            .unwrap();
        assert_eq!(session.state().device.now_ms, 0);
        session.command(Command::Advance { ms: 30_250 }).unwrap();
        assert_eq!(session.state().device.heartbeats, 1);
        session.command(Command::Reset).unwrap();
        assert_eq!(session.state().device.now_ms, 0);
        assert_eq!(session.state().generation, 1);
        assert!(!session.state().running);
    }
}
