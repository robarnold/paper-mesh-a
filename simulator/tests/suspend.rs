use embedded_graphics::primitives::Rectangle;
use paper_mesh_app::{
    BACK_TARGET, TAP_TARGET,
    power::{POWER_TARGET, SUSPEND_TAB},
    suspend::{ONE_MINUTE, START, TOUCH_WAKE},
};
use paper_mesh_platform::{
    TouchEvent,
    input::buttons::ButtonId,
    suspend::{SuspendError, WakeReason},
};
use paper_mesh_simulator::{REFRESH_MS, Simulation, power::PowerScenario};

fn press(sim: &mut Simulation, target: Rectangle) {
    sim.touch_portrait(TouchEvent::touch(0, target.center()));
    sim.touch_portrait(TouchEvent::release(0, target.center()));
}

fn start(sim: &mut Simulation) {
    press(sim, POWER_TARGET);
    press(sim, SUSPEND_TAB);
    press(sim, ONE_MINUTE);
    press(sim, START);
    sim.advance(2 * REFRESH_MS);
}

#[test]
fn timed_suspend_retains_state_and_suppresses_periodic_work() {
    let mut sim = Simulation::default();
    press(&mut sim, TAP_TARGET);
    start(&mut sim);
    assert!(sim.snapshot().suspended);
    let frames = sim.snapshot().completed_refreshes;
    let heartbeats = sim.snapshot().heartbeats;
    sim.advance(59_000);
    assert!(sim.snapshot().suspended);
    assert_eq!(sim.snapshot().completed_refreshes, frames);
    assert_eq!(sim.snapshot().heartbeats, heartbeats);
    sim.advance(1_000);
    assert!(!sim.snapshot().suspended);
    assert_eq!(sim.snapshot().taps, 1);
    let report = sim.power().suspend.result().unwrap().unwrap();
    assert_eq!(
        report.wake_reasons,
        [Some(WakeReason::Timer), None, None, None]
    );
    assert_eq!(report.elapsed_ms, 60_000);
    assert_eq!(report.charge_loss_mah(), Some(0));
    assert_eq!(sim.snapshot().heartbeats, heartbeats + 1);
    press(&mut sim, BACK_TARGET);
    press(&mut sim, BACK_TARGET);
    press(&mut sim, TAP_TARGET);
    assert_eq!(sim.snapshot().taps, 2);
}

#[test]
fn function_wakes_early_and_charge_gain_keeps_its_sign() {
    let mut sim = Simulation::default();
    start(&mut sim);
    sim.advance(4_000);
    sim.set_power_scenario(PowerScenario::Full);
    sim.hardware_button(2, true).unwrap();
    assert!(sim.snapshot().suspended);
    sim.hardware_button(1, true).unwrap();
    let report = sim.power().suspend.result().unwrap().unwrap();
    assert_eq!(
        report.wake_reasons[0],
        Some(WakeReason::Control(ButtonId(1)))
    );
    assert!(report.elapsed_ms < 60_000);
    assert_eq!(report.charge_loss_mah(), Some(-902));
    assert_eq!(report.external_power_at_endpoints(), Some(true));
}

#[test]
fn gauge_failure_does_not_become_zero_charge_loss() {
    let mut sim = Simulation::default();
    start(&mut sim);
    sim.set_power_scenario(PowerScenario::GaugeFault);
    sim.advance(60_000);
    let report = sim.power().suspend.result().unwrap().unwrap();
    assert_eq!(report.charge_loss_mah(), None);
    assert_eq!(report.external_power_at_endpoints(), Some(false));
}

#[test]
fn stale_samples_and_missing_batteries_do_not_produce_a_charge_comparison() {
    for scenario in [
        PowerScenario::Stale,
        PowerScenario::NoBattery,
        PowerScenario::Initializing,
        PowerScenario::Unavailable,
    ] {
        let mut sim = Simulation::default();
        sim.set_power_scenario(scenario);
        start(&mut sim);
        sim.advance(60_000);
        assert_eq!(
            sim.power()
                .suspend
                .result()
                .unwrap()
                .unwrap()
                .charge_loss_mah(),
            None
        );
    }
}

#[test]
fn held_wake_control_rejects_sleep_and_allows_retry() {
    let mut sim = Simulation::default();
    sim.hardware_button(1, true).unwrap();
    start(&mut sim);
    assert!(!sim.snapshot().suspended);
    assert_eq!(
        sim.power().suspend.result(),
        Some(Err(SuspendError::WakeActive))
    );
    sim.hardware_button(1, false).unwrap();
    press(&mut sim, START);
    sim.advance(2 * REFRESH_MS);
    assert!(sim.snapshot().suspended);
}

#[test]
fn time_step_size_does_not_change_suspend_duration_or_result() {
    let mut large = Simulation::default();
    let mut small = Simulation::default();
    start(&mut large);
    start(&mut small);
    large.advance(65_000);
    for _ in 0..260 {
        small.advance(250);
    }
    assert_eq!(
        large.power().suspend.result(),
        small.power().suspend.result()
    );
    assert_eq!(large.display().pixels(), small.display().pixels());
    assert_eq!(large.snapshot().heartbeats, small.snapshot().heartbeats);
}

#[test]
fn boot_wakes_and_repeated_cycles_keep_distinct_causes() {
    let mut sim = Simulation::default();
    start(&mut sim);
    for id in [0, 1, 0] {
        sim.hardware_button(id, true).unwrap();
        let report = sim.power().suspend.result().unwrap().unwrap();
        assert_eq!(
            report.wake_reasons[0],
            Some(WakeReason::Control(ButtonId(id)))
        );
        sim.hardware_button(id, false).unwrap();
        press(&mut sim, START);
        sim.advance(2 * REFRESH_MS);
        assert!(sim.snapshot().suspended);
    }
    sim.advance(60_000);
    assert_eq!(sim.snapshot().suspend_wake_reasons, ["Timer"]);
}

#[test]
fn touch_wake_is_optional_and_consumes_the_wake_contact() {
    let mut sim = Simulation::default();
    start(&mut sim);
    assert!(!sim.snapshot().suspend_touch_wake);
    press(&mut sim, START);
    assert!(sim.snapshot().suspended);
    sim.hardware_button(0, true).unwrap();
    sim.hardware_button(0, false).unwrap();
    press(&mut sim, TOUCH_WAKE);
    press(&mut sim, START);
    sim.advance(2 * REFRESH_MS);
    assert!(sim.snapshot().suspended);
    let at = START.center();
    sim.touch_portrait(TouchEvent::touch(0, at));
    assert!(!sim.snapshot().suspended);
    assert_eq!(sim.snapshot().suspend_wake_reasons, ["Touch/Home IRQ"]);
    // Movement and release must not start another experiment.
    sim.touch_portrait(TouchEvent::touch(0, at));
    sim.touch_portrait(TouchEvent::release(0, at));
    sim.advance(2 * REFRESH_MS);
    assert!(!sim.snapshot().suspended);
    press(&mut sim, START);
    sim.advance(2 * REFRESH_MS);
    assert!(sim.snapshot().suspended);
    sim.hardware_button(2, true).unwrap();
    assert_eq!(sim.snapshot().suspend_wake_reasons, ["Touch/Home IRQ"]);
}

#[test]
fn boot_and_enabled_home_or_touch_must_be_released_before_sleep() {
    for id in [0, 2] {
        let mut sim = Simulation::default();
        press(&mut sim, POWER_TARGET);
        press(&mut sim, SUSPEND_TAB);
        press(&mut sim, TOUCH_WAKE);
        sim.hardware_button(id, true).unwrap();
        press(&mut sim, START);
        sim.advance(2 * REFRESH_MS);
        assert_eq!(
            sim.power().suspend.result(),
            Some(Err(SuspendError::WakeActive))
        );
    }
    let mut sim = Simulation::default();
    press(&mut sim, POWER_TARGET);
    press(&mut sim, SUSPEND_TAB);
    press(&mut sim, TOUCH_WAKE);
    press(&mut sim, START);
    sim.touch_portrait(TouchEvent::touch(1, START.center()));
    sim.advance(2 * REFRESH_MS);
    assert_eq!(
        sim.power().suspend.result(),
        Some(Err(SuspendError::WakeActive))
    );
}

#[path = "../../firmware/src/suspend/wake.rs"]
mod wake;

#[test]
fn firmware_preserves_simultaneous_events_and_unknown_expander_inputs() {
    assert_eq!(
        wake::reasons(wake::BOOT | wake::EXPANDER | wake::TOUCH, Some(0xfb), true),
        [
            Some(WakeReason::Timer),
            Some(WakeReason::Control(ButtonId(0))),
            Some(WakeReason::Control(ButtonId(1))),
            Some(WakeReason::TouchInterrupt),
        ]
    );
    for port in [Some(0xff), None] {
        assert_eq!(
            wake::reasons(wake::EXPANDER, port, false),
            [None, None, Some(WakeReason::UnidentifiedInput), None]
        );
    }
    assert_eq!(
        wake::reasons(0, Some(0xfb), true),
        [Some(WakeReason::Timer), None, None, None]
    );
}
