//! Export the normal diagnostics flow for layout review.

use paper_mesh_app::diagnostics::{HOME_TARGET, START};
use paper_mesh_platform::TouchEvent;
use paper_mesh_simulator::{REFRESH_MS, Simulation};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/diagnostics".into());
    std::fs::create_dir_all(&directory)?;
    let mut sim = Simulation::default();
    sim.advance(REFRESH_MS);
    std::fs::write(format!("{directory}/home.png"), sim.portrait_png()?)?;
    for (target, name) in [(HOME_TARGET, "menu"), (START, "black")] {
        sim.touch_portrait(TouchEvent::touch(0, target.center()));
        sim.touch_portrait(TouchEvent::release(0, target.center()));
        sim.advance(REFRESH_MS);
        std::fs::write(format!("{directory}/{name}.png"), sim.portrait_png()?)?;
    }
    for name in ["white", "gray", "complete"] {
        sim.advance(1);
        sim.hardware_button(0, true)?;
        sim.advance(30);
        sim.hardware_button(0, false)?;
        sim.advance(REFRESH_MS);
        std::fs::write(format!("{directory}/{name}.png"), sim.portrait_png()?)?;
    }
    Ok(())
}
