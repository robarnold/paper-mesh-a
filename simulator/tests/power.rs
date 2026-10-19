use embedded_graphics::primitives::Rectangle;
use paper_mesh_app::{
    BACK_TARGET, TAP_TARGET,
    power::{BATTERY_TAB, CHARGER_TAB, POWER_MAX_AGE_MS, POWER_TARGET, ReadingStatus},
};
use paper_mesh_platform::{
    TouchEvent,
    power::{ChargeState, GaugeState, PowerSource},
};
use paper_mesh_simulator::{REFRESH_MS, Simulation, power::PowerScenario};

fn press(sim: &mut Simulation, target: Rectangle) {
    sim.touch_portrait(TouchEvent::Down(target.center()));
    sim.touch_portrait(TouchEvent::Up(target.center()));
}

#[test]
fn power_navigation_and_supply_transitions_use_the_shared_application() {
    let mut sim = Simulation::default();
    sim.advance(REFRESH_MS);
    let home = sim.display().pixels().to_vec();
    press(&mut sim, POWER_TARGET);
    assert!(sim.power().is_open());
    assert_eq!(sim.power().view().battery.unwrap().current_ma, Some(-180));
    sim.advance(REFRESH_MS);
    assert_ne!(sim.display().pixels(), home);
    let battery_frame = sim.display().pixels().to_vec();
    press(&mut sim, CHARGER_TAB);
    sim.advance(REFRESH_MS);
    assert!(sim.power().charger_tab());
    assert_ne!(sim.display().pixels(), battery_frame);
    sim.set_power_scenario(PowerScenario::Charging);
    let view = sim.power().view();
    assert_eq!(view.battery.unwrap().current_ma, Some(640));
    assert_eq!(view.charger.unwrap().charge, ChargeState::Fast);
    assert_eq!(view.charger.unwrap().source, PowerSource::Usb);
    sim.set_power_scenario(PowerScenario::Full);
    assert_eq!(sim.snapshot().battery_percent, Some(100));
    assert_eq!(
        sim.power().view().charger.unwrap().charge,
        ChargeState::Complete
    );
    press(&mut sim, BATTERY_TAB);
    assert!(!sim.power().charger_tab());
    press(&mut sim, BACK_TARGET);
    assert!(!sim.power().is_open());
    press(&mut sim, TAP_TARGET);
    assert_eq!(sim.snapshot().taps, 1);
}

#[test]
fn independent_failures_absence_and_startup_hide_only_affected_values() {
    let mut sim = Simulation::default();
    press(&mut sim, POWER_TARGET);
    sim.set_power_scenario(PowerScenario::GaugeFault);
    assert_eq!(sim.power().view().battery_status, ReadingStatus::Fault);
    assert!(sim.power().view().battery.is_none());
    assert!(sim.power().view().charger.is_some());
    sim.set_power_scenario(PowerScenario::ChargerFault);
    assert_eq!(sim.power().view().charger_status, ReadingStatus::Fault);
    assert!(sim.power().view().battery.is_some());
    assert!(sim.power().view().charger.is_none());
    sim.set_power_scenario(PowerScenario::NoBattery);
    assert_eq!(sim.snapshot().battery_present, Some(false));
    assert_eq!(sim.snapshot().battery_percent, None);
    assert_eq!(sim.snapshot().battery_voltage_mv, None);
    assert_eq!(sim.power().view().charger.unwrap().source, PowerSource::Usb);
    sim.set_power_scenario(PowerScenario::Initializing);
    assert_eq!(
        sim.power().view().battery.unwrap().gauge,
        GaugeState::Initializing
    );
    assert_eq!(sim.snapshot().battery_percent, None);
    sim.set_power_scenario(PowerScenario::Unavailable);
    assert_eq!(
        sim.power().view().battery_status,
        ReadingStatus::Unavailable
    );
    assert!(sim.power().view().battery.is_none());
    sim.set_power_scenario(PowerScenario::TemperatureFault);
    assert!(sim.power().view().battery.unwrap().charge_inhibited);
    sim.set_power_scenario(PowerScenario::Battery);
    assert_eq!(sim.snapshot().battery_percent, Some(68));
}

#[test]
fn expiry_and_frames_do_not_depend_on_time_step_size() {
    let mut large = Simulation::default();
    let mut small = Simulation::default();
    for sim in [&mut large, &mut small] {
        sim.set_power_scenario(PowerScenario::Stale);
        press(sim, POWER_TARGET);
    }
    large.advance(POWER_MAX_AGE_MS - 1);
    assert_eq!(large.power().view().battery_status, ReadingStatus::Fresh);
    large.advance(1);
    assert_eq!(large.power().view().battery_status, ReadingStatus::Stale);
    assert_eq!(large.power().view().charger_status, ReadingStatus::Stale);
    assert_eq!(large.snapshot().battery_percent, None);
    large.advance(REFRESH_MS);
    for _ in 0..41 {
        small.advance(250);
    }
    assert_eq!(large.power().view(), small.power().view());
    assert_eq!(large.display().pixels(), small.display().pixels());
    assert_eq!(
        large.snapshot().completed_refreshes,
        small.snapshot().completed_refreshes
    );
    large.set_power_scenario(PowerScenario::Charging);
    assert_eq!(large.power().view().battery_status, ReadingStatus::Fresh);
}

#[test]
fn hidden_power_changes_do_not_refresh_the_home_screen() {
    let mut sim = Simulation::default();
    sim.advance(REFRESH_MS);
    let before = sim.snapshot().completed_refreshes;
    for scenario in [
        PowerScenario::Charging,
        PowerScenario::LowBattery,
        PowerScenario::GaugeFault,
        PowerScenario::Battery,
    ] {
        sim.set_power_scenario(scenario);
        sim.advance(2000);
    }
    assert_eq!(sim.snapshot().completed_refreshes, before);
    sim.touch_portrait(TouchEvent::Down(POWER_TARGET.center()));
    sim.touch_portrait(TouchEvent::Cancel);
    sim.touch_portrait(TouchEvent::Up(POWER_TARGET.center()));
    assert!(!sim.power().is_open());
}
