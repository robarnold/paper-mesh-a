use embedded_graphics::primitives::Rectangle;
use paper_mesh_app::{
    BACK_TARGET, TAP_TARGET,
    diagnostics::{Component, HOME_TARGET},
    display_check::START,
};
use paper_mesh_platform::TouchEvent;
use paper_mesh_simulator::{REFRESH_MS, Simulation};

fn tap(sim: &mut Simulation, target: Rectangle) {
    sim.touch_portrait(TouchEvent::touch(0, target.center()));
    sim.touch_portrait(TouchEvent::release(0, target.center()));
}

fn open(sim: &mut Simulation) {
    tap(sim, HOME_TARGET);
    tap(sim, Component::Display.target());
    sim.advance(2 * REFRESH_MS);
    assert!(sim.snapshot().diagnostics_open);
    assert_eq!(sim.snapshot().diagnostic_pattern, None);
}

fn advance(sim: &mut Simulation) {
    sim.advance(1);
    sim.hardware_button(0, true).unwrap();
    sim.advance(30);
    sim.hardware_button(0, false).unwrap();
}

#[test]
fn normal_application_runs_full_patterns_and_holds_them_without_redraws() {
    let mut sim = Simulation::default();
    open(&mut sim);
    tap(&mut sim, START);
    for (index, name, value) in [(1, "black", 0), (2, "white", 255), (3, "gray", 136)] {
        assert!(!sim.snapshot().diagnostic_ready);
        sim.advance(REFRESH_MS);
        let state = sim.snapshot();
        assert_eq!(state.diagnostic_pattern, Some(name));
        assert!(state.controls.is_empty());
        assert!(state.diagnostic_ready);
        assert_eq!(state.full_refreshes, index);
        assert!(sim.display().pixels().iter().all(|pixel| *pixel == value));
        let image = sim.portrait_png().unwrap();
        tap(&mut sim, BACK_TARGET);
        tap(&mut sim, TAP_TARGET);
        sim.advance(120_000);
        assert_eq!(
            sim.snapshot().completed_refreshes,
            state.completed_refreshes
        );
        assert_eq!(sim.portrait_png().unwrap(), image);
        advance(&mut sim);
    }
    sim.advance(REFRESH_MS);
    assert!(sim.snapshot().diagnostics_open);
    assert_eq!(sim.snapshot().diagnostic_pattern, None);
    assert_eq!(sim.snapshot().full_refreshes, 4);
    tap(&mut sim, BACK_TARGET);
    sim.advance(REFRESH_MS);
    assert!(sim.snapshot().diagnostics_open);
    tap(&mut sim, Component::Touch.target());
    tap(&mut sim, TAP_TARGET);
    assert_eq!(sim.snapshot().taps, 1);
}

#[test]
fn press_during_refresh_and_a_long_hold_cannot_skip_patterns_or_power_off() {
    let mut sim = Simulation::default();
    open(&mut sim);
    tap(&mut sim, START);
    sim.hardware_button(0, true).unwrap();
    sim.advance(REFRESH_MS + 3_500);
    sim.hardware_button(0, false).unwrap();
    assert_eq!(sim.snapshot().diagnostic_pattern, Some("black"));
    assert!(!sim.snapshot().powered_off);
    sim.advance(1);
    sim.hardware_button(0, true).unwrap();
    sim.advance(5_000);
    assert!(!sim.snapshot().powered_off);
    assert_eq!(sim.snapshot().diagnostic_pattern, Some("black"));
    sim.hardware_button(0, false).unwrap();
    sim.advance(REFRESH_MS);
    assert_eq!(sim.snapshot().diagnostic_pattern, Some("white"));
}

#[test]
fn exits_during_refresh_restore_the_menu_or_home_with_a_full_refresh() {
    for id in [1, 2] {
        let mut sim = Simulation::default();
        open(&mut sim);
        tap(&mut sim, START);
        sim.hardware_button(id, true).unwrap();
        sim.hardware_button(id, false).unwrap();
        sim.advance(2 * REFRESH_MS);
        assert_eq!(sim.snapshot().diagnostics_open, id == 1);
        assert!(!sim.snapshot().diagnostic_ready);
        assert_eq!(sim.snapshot().full_refreshes, 2);
        if id == 2 {
            tap(&mut sim, HOME_TARGET);
        }
        tap(&mut sim, Component::Touch.target());
        tap(&mut sim, TAP_TARGET);
        assert_eq!(sim.snapshot().taps, 1);
        sim.hardware_button(0, true).unwrap();
        sim.advance(3_000 + 2 * REFRESH_MS);
        assert!(sim.snapshot().powered_off);
    }
}
