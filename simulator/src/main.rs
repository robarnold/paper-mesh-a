use std::{
    env, fs,
    io::{Cursor, Read},
    path::Path,
    time::{Duration, Instant},
};

use embedded_graphics::prelude::*;
use paper_mesh_platform::TouchEvent;
use paper_mesh_simulator::SCREEN_SIZE;
use paper_mesh_simulator::discovery::ScanScenario;
use paper_mesh_simulator::location::LocationScenario;
use paper_mesh_simulator::power::PowerScenario;
use paper_mesh_simulator::{REFRESH_MS, Simulation, Snapshot};
use serde::{Deserialize, Serialize};
use tiny_http::{Header, Method, Request, Response, Server};

type Error = Box<dyn std::error::Error + Send + Sync>;
type HttpResponse = Response<Cursor<Vec<u8>>>;

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    HardwareButton { id: u8, pressed: bool },
    TouchDown { x: i32, y: i32 },
    TouchMove { x: i32, y: i32 },
    TouchUp { x: i32, y: i32 },
    TouchCancel,
    Pause,
    Resume,
    Advance { ms: u64 },
    Reset,
    ScanScenario { scenario: ScanScenario },
    LocationScenario { scenario: LocationScenario },
    PowerScenario { scenario: PowerScenario },
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
            Command::HardwareButton { id, pressed } => {
                self.simulation.hardware_button(id, pressed)?
            }
            Command::TouchDown { x, y }
            | Command::TouchMove { x, y }
            | Command::TouchUp { x, y } => {
                if x < 0 || y < 0 || x >= SCREEN_SIZE.width as i32 || y >= SCREEN_SIZE.height as i32
                {
                    return Err("Touch must be inside the display.");
                }
                let point = Point::new(x, y);
                let event = if !matches!(command, Command::TouchUp { .. }) {
                    TouchEvent::touch(0, point)
                } else {
                    TouchEvent::release(0, point)
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
    Response::from_data(body)
        .with_status_code(status)
        .with_header(Header::from_bytes("Content-Type", content_type).unwrap())
        .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
        .with_header(Header::from_bytes("X-Content-Type-Options", "nosniff").unwrap())
}

fn handle(request: &mut Request, session: &mut Session) -> HttpResponse {
    match (request.method(), request.url()) {
        (&Method::Get, "/") => response(
            200,
            "text/html; charset=utf-8",
            include_bytes!("index.html").to_vec(),
        ),
        (&Method::Get, "/api/state") => response(
            200,
            "application/json",
            serde_json::to_vec(&session.state())
                .expect("state contains only JSON-compatible values"),
        ),
        (&Method::Get, path @ ("/frame.png" | "/frame-native.png")) => {
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
        (&Method::Post, "/api/event") => {
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
            let command = match serde_json::from_slice::<Command>(&body) {
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

fn main() -> Result<(), Error> {
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
    let server = Server::http("0.0.0.0:8080")?;
    let mut session = Session {
        simulation: Simulation::default(),
        running: true,
        generation: 0,
    };
    let mut last_wall = Instant::now();
    let mut remainder = Duration::ZERO;
    println!("Paper Mesh simulator: http://localhost:8080");
    println!("The device application, rendering, and virtual hardware run in this Rust process.");
    loop {
        let wall = Instant::now();
        if session.running {
            remainder += wall.duration_since(last_wall);
            let milliseconds = remainder.as_millis().min(u64::MAX as u128) as u64;
            remainder -= Duration::from_millis(milliseconds);
            session.simulation.advance(milliseconds);
        } else {
            remainder = Duration::ZERO;
        }
        last_wall = wall;
        if let Some(mut request) = server.recv_timeout(Duration::from_millis(10))? {
            let was_running = session.running;
            let generation = session.generation;
            let result = handle(&mut request, &mut session);
            if let Err(error) = request.respond(result) {
                eprintln!("Client disconnected: {error}");
            }
            if was_running != session.running || generation != session.generation {
                last_wall = Instant::now();
                remainder = Duration::ZERO;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                .command(Command::TouchDown { x: 540, y: 20 })
                .is_err()
        );
        assert!(
            session
                .command(Command::TouchDown { x: 20, y: 960 })
                .is_err()
        );
        assert!(session.command(Command::TouchUp { x: -1, y: 20 }).is_err());
        session
            .command(Command::TouchDown { x: 539, y: 959 })
            .unwrap();
        session
            .command(Command::TouchUp { x: 539, y: 959 })
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
