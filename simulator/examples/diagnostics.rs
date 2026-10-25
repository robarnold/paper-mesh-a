//! Export the normal diagnostics flow for layout review.

use paper_mesh_app::{
    BACK_TARGET, TAP_TARGET,
    diagnostics::{Component, HOME_TARGET},
    display_check::START,
};
use paper_mesh_platform::TouchEvent;
use paper_mesh_simulator::{REFRESH_MS, Simulation};

fn tap(sim: &mut Simulation, target: embedded_graphics::primitives::Rectangle) {
    sim.touch_portrait(TouchEvent::touch(0, target.center()));
    sim.touch_portrait(TouchEvent::release(0, target.center()));
    sim.advance(2 * REFRESH_MS);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/diagnostics".into());
    std::fs::create_dir_all(&directory)?;
    let mut sim = Simulation::default();
    sim.advance(REFRESH_MS);
    std::fs::write(format!("{directory}/home.png"), sim.portrait_png()?)?;
    for (target, name) in [
        (HOME_TARGET, "menu"),
        (Component::Display.target(), "display"),
        (START, "black"),
    ] {
        tap(&mut sim, target);
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
    tap(&mut sim, BACK_TARGET);
    for (component, name) in [
        (Component::Touch, "touch"),
        (Component::Radio, "radio"),
        (Component::Location, "gnss"),
        (Component::Power, "power"),
    ] {
        tap(&mut sim, component.target());
        if component == Component::Touch {
            tap(&mut sim, TAP_TARGET);
        }
        sim.advance(5_000);
        std::fs::write(format!("{directory}/{name}.png"), sim.portrait_png()?)?;
        if component == Component::Power {
            tap(&mut sim, paper_mesh_app::power::CHARGER_TAB);
            std::fs::write(format!("{directory}/charger.png"), sim.portrait_png()?)?;
            tap(&mut sim, paper_mesh_app::power::SUSPEND_TAB);
            std::fs::write(format!("{directory}/suspend.png"), sim.portrait_png()?)?;
            tap(&mut sim, BACK_TARGET);
        }
        tap(&mut sim, BACK_TARGET);
    }
    Ok(())
}
