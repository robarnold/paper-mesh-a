use embedded_graphics::primitives::Rectangle;
use paper_mesh_app::diagnostics::{Component, HOME_TARGET};
use paper_mesh_app::{BACK_TARGET, TAP_TARGET, power::OFF_TAB, power_off::START};
use paper_mesh_platform::{TouchEvent, power_off::PowerOffError};
use paper_mesh_simulator::{REFRESH_MS, Simulation, power::PowerScenario};

fn press(sim: &mut Simulation, target: Rectangle) {
    sim.touch_portrait(TouchEvent::touch(0, target.center()));
    sim.touch_portrait(TouchEvent::release(0, target.center()));
}

fn open(sim: &mut Simulation) {
    press(sim, HOME_TARGET);
    press(sim, Component::Power.target());
    press(sim, OFF_TAB);
    sim.advance(2 * REFRESH_MS);
}

#[test]
fn power_off_requires_explicit_start_and_stops_work_after_final_frame() {
    let mut sim = Simulation::default();
    open(&mut sim);
    assert!(!sim.snapshot().powered_off);
    press(&mut sim, START);
    assert!(!sim.snapshot().powered_off);
    sim.advance(2 * REFRESH_MS);
    assert!(sim.snapshot().powered_off);
    let frames = sim.snapshot().completed_refreshes;
    let heartbeats = sim.snapshot().heartbeats;
    let image = sim.portrait_png().unwrap();
    press(&mut sim, BACK_TARGET);
    press(&mut sim, TAP_TARGET);
    sim.hardware_button(0, true).unwrap();
    sim.advance(3_600_000);
    assert!(sim.snapshot().powered_off);
    assert_eq!(sim.snapshot().completed_refreshes, frames);
    assert_eq!(sim.snapshot().heartbeats, heartbeats);
    assert_eq!(sim.portrait_png().unwrap(), image);
}

#[test]
fn external_power_refusal_keeps_controls_usable_and_allows_retry() {
    let mut sim = Simulation::default();
    sim.set_power_scenario(PowerScenario::Charging);
    open(&mut sim);
    press(&mut sim, START);
    sim.advance(3 * REFRESH_MS);
    assert!(!sim.snapshot().powered_off);
    assert_eq!(sim.power().off.error(), Some(PowerOffError::ExternalPower));
    sim.set_power_scenario(PowerScenario::Battery);
    press(&mut sim, START);
    sim.advance(3 * REFRESH_MS);
    assert!(sim.snapshot().powered_off);
}

#[test]
fn missing_or_expired_charger_observation_cannot_approve_power_off() {
    for scenario in [
        PowerScenario::ChargerFault,
        PowerScenario::Unavailable,
        PowerScenario::Stale,
    ] {
        let mut sim = Simulation::default();
        sim.set_power_scenario(scenario);
        sim.advance(20_000);
        open(&mut sim);
        press(&mut sim, START);
        sim.advance(3 * REFRESH_MS);
        assert!(!sim.snapshot().powered_off);
        assert_eq!(sim.power().off.error(), Some(PowerOffError::Preparation));
    }
}

#[test]
fn boot_hold_requests_power_off_but_short_press_does_not() {
    let mut sim = Simulation::default();
    sim.hardware_button(0, true).unwrap();
    sim.advance(2_999);
    assert!(!sim.snapshot().powered_off);
    sim.hardware_button(0, false).unwrap();
    sim.advance(5_000);
    assert!(!sim.power().off.open);
    sim.hardware_button(0, true).unwrap();
    sim.advance(3_000 + 2 * REFRESH_MS);
    assert!(sim.snapshot().powered_off);
}
