use paper_mesh_platform::{
    Clock,
    time::{Date, Month, Time, UtcDateTime},
};
use paper_mesh_simulator::{Simulation, location::LocationScenario};

#[test]
fn gps_sets_calendar_time_and_receiver_loss_does_not_stop_it() {
    let mut sim = Simulation::default();
    assert_eq!(sim.clock().now_utc(), None);
    sim.advance(3_000);
    assert_eq!(
        sim.clock().now_utc(),
        Some(UtcDateTime::new(
            Date::from_calendar_date(2020, Month::April, 25).unwrap(),
            Time::from_hms(7, 15, 59).unwrap(),
        ))
    );
    sim.set_location_scenario(LocationScenario::Unavailable);
    sim.advance(2_000);
    assert_eq!(sim.clock().now_utc().unwrap().as_hms(), (7, 16, 1));
    sim.set_utc(None);
    sim.advance(1_000);
    assert_eq!(sim.clock().now_utc(), None);
}

#[test]
fn calendar_corrections_do_not_change_deadlines_or_display_completion() {
    let mut changed = Simulation::default();
    let mut baseline = Simulation::default();
    changed.set_location_scenario(LocationScenario::Unavailable);
    baseline.set_location_scenario(LocationScenario::Unavailable);
    for year in [2026, 2020, 2099] {
        changed.set_utc(Some(UtcDateTime::new(
            Date::from_calendar_date(year, Month::December, 31).unwrap(),
            Time::from_hms(23, 59, 59).unwrap(),
        )));
        changed.advance(1_000);
        baseline.advance(1_000);
        assert_eq!(changed.clock().now_utc().unwrap().year(), year + 1);
        assert_eq!(changed.clock().now_ms(), baseline.clock().now_ms());
        assert_eq!(
            changed.snapshot().heartbeats,
            baseline.snapshot().heartbeats
        );
        assert_eq!(changed.display().pixels(), baseline.display().pixels());
    }
}
