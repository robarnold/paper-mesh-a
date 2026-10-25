//! Export the power-off instructions for layout review.

use paper_mesh_app::diagnostics::{Component, HOME_TARGET};
use paper_mesh_app::{power::OFF_TAB, power_off::START};
use paper_mesh_platform::TouchEvent;
use paper_mesh_simulator::{REFRESH_MS, Simulation, power::PowerScenario};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/power-off.png".into());
    let mut sim = Simulation::default();
    sim.set_power_scenario(PowerScenario::Charging);
    for target in [HOME_TARGET, Component::Power.target(), OFF_TAB] {
        sim.touch_portrait(TouchEvent::touch(0, target.center()));
        sim.touch_portrait(TouchEvent::release(0, target.center()));
        sim.advance(2 * REFRESH_MS);
    }
    std::fs::write(&path, sim.portrait_png()?)?;
    sim.touch_portrait(TouchEvent::touch(0, START.center()));
    sim.touch_portrait(TouchEvent::release(0, START.center()));
    sim.advance(3 * REFRESH_MS);
    std::fs::write(path.replace(".png", "-refusal.png"), sim.portrait_png()?)?;
    Ok(())
}
