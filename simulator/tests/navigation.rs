use embedded_graphics::primitives::Rectangle;
use paper_mesh_app::{
    BACK_TARGET, TAP_TARGET,
    diagnostics::{Component, HOME_TARGET},
    power::{CHARGER_TAB, OFF_TAB, SUSPEND_TAB},
    touchpad::TOUCHPAD_TARGET,
};
use paper_mesh_platform::TouchEvent;
use paper_mesh_simulator::{REFRESH_MS, Simulation, power::PowerScenario};

fn tap(sim: &mut Simulation, target: Rectangle) {
    sim.touch_portrait(TouchEvent::touch(0, target.center()));
    sim.touch_portrait(TouchEvent::release(0, target.center()));
}

fn home(sim: &mut Simulation) {
    sim.hardware_button(2, true).unwrap();
    sim.hardware_button(2, false).unwrap();
}

#[test]
fn home_suppresses_held_contacts_and_does_not_redraw_the_home_screen() {
    let mut sim = Simulation::default();
    sim.advance(REFRESH_MS);
    let refreshes = sim.snapshot().completed_refreshes;
    home(&mut sim);
    sim.advance(REFRESH_MS);
    assert_eq!(sim.snapshot().completed_refreshes, refreshes);
    tap(&mut sim, HOME_TARGET);
    tap(&mut sim, Component::Touch.target());
    sim.touch_portrait(TouchEvent::touch(0, TAP_TARGET.center()));
    home(&mut sim);
    sim.touch_portrait(TouchEvent::touch(0, HOME_TARGET.center()));
    sim.touch_portrait(TouchEvent::release(0, HOME_TARGET.center()));
    assert!(!sim.snapshot().diagnostics_open);
    assert_eq!(sim.snapshot().taps, 0);
    tap(&mut sim, HOME_TARGET);
    assert!(sim.snapshot().diagnostics_open);
}

#[test]
fn home_returns_directly_from_each_component_and_power_subpage() {
    let mut sim = Simulation::default();
    for (component, subpage) in Component::ALL
        .into_iter()
        .map(|component| (component, None))
        .chain([
            (Component::Power, Some(SUSPEND_TAB)),
            (Component::Power, Some(OFF_TAB)),
        ])
    {
        tap(&mut sim, HOME_TARGET);
        tap(&mut sim, component.target());
        if let Some(target) = subpage {
            tap(&mut sim, target);
        }
        home(&mut sim);
        let state = sim.snapshot();
        assert!(!state.diagnostics_open);
        assert_eq!(state.diagnostic_component, None);
        assert!(!sim.debugger().is_open());
        assert!(!sim.location().is_open());
        assert!(!sim.power().is_open());
        assert!(
            state
                .controls
                .iter()
                .all(|control| !["Back", "Home"].contains(&control.label))
        );
        sim.advance(2_000);
        assert_eq!(sim.snapshot().diagnostic_component, None);
    }
}

#[test]
fn back_requires_a_complete_tap_and_home_preserves_the_counter() {
    let mut sim = Simulation::default();
    tap(&mut sim, HOME_TARGET);
    tap(&mut sim, Component::Touch.target());
    sim.touch_portrait(TouchEvent::touch(0, BACK_TARGET.center()));
    sim.touch_portrait(TouchEvent::release(
        0,
        embedded_graphics::prelude::Point::new(300, 52),
    ));
    assert_eq!(sim.snapshot().diagnostic_component, Some("Touch"));
    sim.touch_portrait(TouchEvent::touch(0, BACK_TARGET.center()));
    sim.touch_portrait(TouchEvent::Cancel);
    sim.touch_portrait(TouchEvent::release(0, BACK_TARGET.center()));
    assert_eq!(sim.snapshot().diagnostic_component, Some("Touch"));
    tap(&mut sim, TAP_TARGET);
    home(&mut sim);
    tap(&mut sim, HOME_TARGET);
    tap(&mut sim, Component::Touch.target());
    assert_eq!(sim.snapshot().taps, 1);
}

#[test]
fn pending_operations_block_navigation_until_completion() {
    use paper_mesh_app::App;
    use paper_mesh_platform::{power_off::PowerOffError, suspend::SuspendError};

    fn press(app: &mut App, target: Rectangle) {
        app.touch(TouchEvent::touch(0, target.center()));
        app.touch(TouchEvent::release(0, target.center()));
    }

    for suspend in [false, true] {
        let mut app = App::new_at(0);
        app.set_power_off_restart_hint("Restart the simulator.");
        press(&mut app, HOME_TARGET);
        press(&mut app, Component::Power.target());
        press(&mut app, if suspend { SUSPEND_TAB } else { OFF_TAB });
        press(
            &mut app,
            if suspend {
                paper_mesh_app::suspend::START
            } else {
                paper_mesh_app::power_off::START
            },
        );
        assert!(
            app.navigation_controls()
                .iter()
                .all(|control| !control.enabled)
        );
        press(&mut app, BACK_TARGET);
        app.go_home();
        assert_eq!(app.active_diagnostic(), Some(Component::Power));
        app.touch(TouchEvent::touch(0, BACK_TARGET.center()));
        if suspend {
            app.finish_suspend(Err(SuspendError::Preparation));
        } else {
            app.finish_power_off(PowerOffError::ExternalPower);
        }
        app.touch(TouchEvent::release(0, BACK_TARGET.center()));
        assert_eq!(app.active_diagnostic(), Some(Component::Power));
        assert!(
            app.navigation_controls()
                .iter()
                .all(|control| control.enabled)
        );
        app.go_home();
        assert!(!app.diagnostics().is_open());
        assert_eq!(app.active_diagnostic(), None);
    }
}

#[test]
fn components_share_one_menu_and_return_path() {
    let mut sim = Simulation::default();
    assert!(!sim.snapshot().diagnostics_open);
    tap(&mut sim, HOME_TARGET);
    assert!(sim.snapshot().diagnostics_open);
    assert_eq!(sim.snapshot().diagnostic_component, None);
    for component in Component::ALL {
        tap(&mut sim, component.target());
        let state = sim.snapshot();
        let back = state
            .controls
            .iter()
            .find(|control| control.label == "Back")
            .unwrap();
        assert_eq!(
            (back.x, back.y),
            (BACK_TARGET.center().x, BACK_TARGET.center().y)
        );
        assert!(back.enabled);
        assert!(state.controls.iter().all(|control| control.label != "Home"));
        assert_eq!(state.diagnostic_component, Some(component.label()));
        assert_eq!(sim.debugger().is_open(), component == Component::Radio);
        assert_eq!(sim.location().is_open(), component == Component::Location);
        assert_eq!(sim.power().is_open(), component == Component::Power);
        assert_eq!(state.diagnostic_pattern, None);
        assert!(!state.powered_off);
        assert!(!state.suspended);
        tap(&mut sim, BACK_TARGET);
        assert!(sim.snapshot().diagnostics_open);
        assert_eq!(sim.snapshot().diagnostic_component, None);
    }
    tap(&mut sim, BACK_TARGET);
    assert!(!sim.snapshot().diagnostics_open);
    tap(&mut sim, TOUCHPAD_TARGET);
    assert!(sim.snapshot().touchpad_open);
    assert!(sim.snapshot().controls.is_empty());
    sim.hardware_button(1, true).unwrap();
    assert!(!sim.snapshot().touchpad_open);
    assert!(!sim.snapshot().diagnostics_open);
    assert_eq!(sim.snapshot().taps, 0);
}

#[test]
fn menu_requires_fresh_taps_and_retains_the_touch_counter() {
    let mut sim = Simulation::default();
    tap(&mut sim, HOME_TARGET);
    let display = Component::Display.target().center();
    let touch = Component::Touch.target().center();
    sim.touch_portrait(TouchEvent::release(0, display));
    sim.touch_portrait(TouchEvent::touch(0, display));
    sim.touch_portrait(TouchEvent::release(0, touch));
    sim.touch_portrait(TouchEvent::touch(0, touch));
    sim.touch_portrait(TouchEvent::Cancel);
    sim.touch_portrait(TouchEvent::release(0, touch));
    assert_eq!(sim.snapshot().diagnostic_component, None);
    tap(&mut sim, Component::Touch.target());
    tap(&mut sim, TAP_TARGET);
    tap(&mut sim, BACK_TARGET);
    tap(&mut sim, BACK_TARGET);
    tap(&mut sim, HOME_TARGET);
    tap(&mut sim, Component::Touch.target());
    assert_eq!(sim.snapshot().taps, 1);
}

#[test]
fn power_subpages_return_through_power_to_the_menu() {
    let mut sim = Simulation::default();
    tap(&mut sim, HOME_TARGET);
    tap(&mut sim, Component::Power.target());
    tap(&mut sim, CHARGER_TAB);
    for target in [SUSPEND_TAB, OFF_TAB] {
        tap(&mut sim, target);
        assert!(sim.power().suspend.open || sim.power().off.open);
        tap(&mut sim, BACK_TARGET);
        assert!(!sim.power().suspend.open);
        assert!(!sim.power().off.open);
        assert!(sim.power().charger_tab());
        assert_eq!(sim.snapshot().diagnostic_component, Some("Power"));
    }
    tap(&mut sim, BACK_TARGET);
    assert_eq!(sim.snapshot().diagnostic_component, None);
    assert!(sim.snapshot().diagnostics_open);
}

#[test]
fn refused_hardware_shutdown_returns_to_the_invoking_component() {
    for component in Component::ALL {
        let mut sim = Simulation::default();
        sim.set_power_scenario(PowerScenario::Charging);
        tap(&mut sim, HOME_TARGET);
        tap(&mut sim, component.target());
        sim.hardware_button(0, true).unwrap();
        sim.advance(3_000 + 2 * REFRESH_MS);
        sim.hardware_button(0, false).unwrap();
        assert!(!sim.snapshot().powered_off);
        assert_eq!(sim.snapshot().diagnostic_component, Some("Power"));
        assert!(!sim.debugger().is_open());
        assert!(!sim.location().is_open());
        tap(&mut sim, BACK_TARGET);
        assert_eq!(sim.snapshot().diagnostic_component, Some(component.label()));
        assert_eq!(sim.debugger().is_open(), component == Component::Radio);
        assert_eq!(sim.location().is_open(), component == Component::Location);
        tap(&mut sim, BACK_TARGET);
        assert_eq!(sim.snapshot().diagnostic_component, None);
        assert!(sim.snapshot().diagnostics_open);
    }
}

#[test]
fn refused_hardware_shutdown_invoked_from_home_returns_directly_home() {
    let mut sim = Simulation::default();
    sim.set_power_scenario(PowerScenario::Charging);
    sim.hardware_button(0, true).unwrap();
    sim.advance(3_000 + 2 * REFRESH_MS);
    sim.hardware_button(0, false).unwrap();
    assert!(!sim.snapshot().powered_off);
    assert!(sim.power().off.open);
    tap(&mut sim, BACK_TARGET);
    assert!(!sim.snapshot().diagnostics_open);
    assert_eq!(sim.snapshot().diagnostic_component, None);
    assert!(!sim.power().is_open());
}
