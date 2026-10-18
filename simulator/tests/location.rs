use embedded_graphics::{prelude::*, primitives::Rectangle};
use paper_mesh_app::{
    TAP_TARGET,
    debugger::{BACK_TARGET, DEBUGGER_TARGET},
    location::{FixStatus, LOCATION_TARGET, ReceiverStatus},
    platform::TouchEvent,
};
use paper_mesh_simulator::{
    REFRESH_MS, Simulation,
    location::{ACQUISITION_MS, LocationScenario},
};

fn press(sim: &mut Simulation, target: Rectangle) {
    sim.touch_portrait(TouchEvent::Down(target.center()));
    sim.touch_portrait(TouchEvent::Up(target.center()));
}

#[test]
fn location_screen_acquires_a_fix_and_keeps_other_screens_available() {
    let mut sim = Simulation::default();
    sim.advance(REFRESH_MS);
    let home = sim.display().pixels().to_vec();
    press(&mut sim, LOCATION_TARGET);
    assert!(sim.location().is_open());
    assert_eq!(sim.location().view().status, FixStatus::Acquiring);
    assert_eq!(sim.location().view().coordinates, None);
    assert_eq!(sim.location().view().elevation_mm, None);
    sim.advance(ACQUISITION_MS - REFRESH_MS - 1);
    assert_eq!(sim.location().view().status, FixStatus::Acquiring);
    sim.advance(1 + REFRESH_MS);
    assert_eq!(sim.location().view().status, FixStatus::Valid);
    assert_eq!(sim.snapshot().latitude_microdegrees, Some(37_774_900));
    assert_eq!(sim.snapshot().longitude_microdegrees, Some(-122_419_400));
    assert_eq!(sim.snapshot().elevation_mm, Some(75_700));
    assert_eq!(sim.snapshot().satellites_used, Some(8));
    assert_ne!(sim.display().pixels(), home);
    press(&mut sim, BACK_TARGET);
    assert!(!sim.location().is_open());
    press(&mut sim, TAP_TARGET);
    assert_eq!(sim.snapshot().taps, 1);
    press(&mut sim, DEBUGGER_TARGET);
    assert!(sim.debugger().is_open());
}

#[test]
fn loss_silence_and_faults_remove_coordinates_and_permit_recovery() {
    let mut sim = Simulation::default();
    sim.set_location_scenario(LocationScenario::LostFix);
    press(&mut sim, LOCATION_TARGET);
    assert_eq!(sim.location().view().status, FixStatus::Valid);
    sim.advance(ACQUISITION_MS);
    assert_eq!(sim.location().view().status, FixStatus::Acquiring);
    assert_eq!(sim.location().view().coordinates, None);
    assert_eq!(sim.location().view().elevation_mm, None);
    sim.set_location_scenario(LocationScenario::Silent);
    sim.advance(4_999);
    assert_eq!(sim.location().view().status, FixStatus::Valid);
    assert_eq!(sim.location().view().elevation_mm, Some(75_700));
    sim.advance(1);
    assert_eq!(sim.location().view().status, FixStatus::Stale);
    assert_eq!(sim.location().view().coordinates, None);
    assert_eq!(sim.location().view().elevation_mm, None);
    assert_eq!(sim.location().view().satellites_used, None);
    assert_eq!(sim.location().view().age_seconds, Some(5));
    for (scenario, status) in [
        (LocationScenario::Unavailable, FixStatus::Unavailable),
        (LocationScenario::Fault, FixStatus::Fault),
        (LocationScenario::Acquiring, FixStatus::Acquiring),
    ] {
        sim.set_location_scenario(scenario);
        assert_eq!(sim.location().view().status, status);
        assert_eq!(sim.location().view().coordinates, None);
        assert_eq!(sim.location().view().elevation_mm, None);
    }
    sim.set_location_scenario(LocationScenario::Fix);
    sim.advance(ACQUISITION_MS);
    assert_eq!(sim.location().view().status, FixStatus::Valid);
}

#[test]
fn zero_coordinates_are_a_valid_fix_and_partial_touches_do_not_navigate() {
    let mut sim = Simulation::default();
    sim.set_location_scenario(LocationScenario::ZeroCoordinates);
    sim.touch_portrait(TouchEvent::Up(LOCATION_TARGET.center()));
    assert!(!sim.location().is_open());
    sim.touch_portrait(TouchEvent::Down(LOCATION_TARGET.center()));
    sim.touch_portrait(TouchEvent::Cancel);
    sim.touch_portrait(TouchEvent::Up(LOCATION_TARGET.center()));
    assert!(!sim.location().is_open());
    press(&mut sim, LOCATION_TARGET);
    assert_eq!(sim.location().view().status, FixStatus::Valid);
    assert_eq!(sim.snapshot().latitude_microdegrees, Some(0));
    assert_eq!(sim.snapshot().longitude_microdegrees, Some(0));
    assert_eq!(sim.snapshot().elevation_mm, Some(0));
    sim.touch_portrait(TouchEvent::Down(BACK_TARGET.center()));
    sim.touch_portrait(TouchEvent::Up(Point::zero()));
    assert!(sim.location().is_open());
    assert_eq!(sim.snapshot().taps, 0);
}

#[test]
fn deadlines_and_completed_frames_match_across_time_step_sizes() {
    let mut large = Simulation::default();
    let mut small = Simulation::default();
    for sim in [&mut large, &mut small] {
        sim.set_location_scenario(LocationScenario::Silent);
        press(sim, LOCATION_TARGET);
    }
    large.advance(8_750);
    for _ in 0..350 {
        small.advance(25);
    }
    assert_eq!(large.location().view(), small.location().view());
    assert_eq!(large.display().pixels(), small.display().pixels());
    assert_eq!(
        large.snapshot().completed_refreshes,
        small.snapshot().completed_refreshes
    );
}

#[test]
fn new_observations_combine_while_display_refresh_is_pending() {
    let mut sim = Simulation::default();
    press(&mut sim, LOCATION_TARGET);
    // The startup frame still owns the panel.
    sim.set_location_scenario(LocationScenario::ZeroCoordinates);
    sim.advance(REFRESH_MS);
    assert!(sim.snapshot().display_busy);
    sim.set_location_scenario(LocationScenario::Fault);
    sim.advance(2 * REFRESH_MS);
    assert_eq!(sim.location().view().status, FixStatus::Fault);
    assert!(!sim.snapshot().display_busy);
    assert_eq!(sim.snapshot().completed_refreshes, 3);
}

#[test]
fn active_receiver_and_signal_levels_do_not_require_a_fix() {
    let mut sim = Simulation::default();
    sim.set_location_scenario(LocationScenario::Acquiring);
    press(&mut sim, LOCATION_TARGET);
    let initial_messages = sim.snapshot().receiver_messages;
    assert_eq!(
        sim.location().view().receiver_status,
        ReceiverStatus::Active
    );
    assert_eq!(sim.snapshot().satellites_in_view, Some(8));
    assert_eq!(sim.snapshot().satellites_with_signal, Some(6));
    assert_eq!(sim.snapshot().satellites_used, Some(0));
    assert_eq!(sim.snapshot().satellite_signals[0].cn0_db_hz, Some(22));
    sim.advance(5_000 + REFRESH_MS);
    assert!(sim.snapshot().receiver_messages > initial_messages);
    assert_eq!(sim.location().view().status, FixStatus::Acquiring);
    assert_eq!(sim.location().view().coordinates, None);
    sim.set_location_scenario(LocationScenario::NoSignal);
    sim.advance(5_000 + REFRESH_MS);
    assert_eq!(sim.snapshot().satellites_in_view, Some(8));
    assert_eq!(sim.snapshot().satellites_with_signal, Some(0));
    assert!(
        sim.snapshot()
            .satellite_signals
            .iter()
            .all(|satellite| satellite.cn0_db_hz.is_none())
    );
}

#[test]
fn response_fix_and_satellite_expiry_are_independent() {
    let mut sim = Simulation::default();
    sim.set_location_scenario(LocationScenario::StaleSatellites);
    press(&mut sim, LOCATION_TARGET);
    sim.advance(4_999);
    assert!(sim.location().view().satellites.is_some());
    sim.advance(1);
    assert!(sim.location().view().satellites.is_none());
    assert_eq!(
        sim.location().view().receiver_status,
        ReceiverStatus::Active
    );
    assert_eq!(sim.location().view().satellite_age_seconds, Some(5));
    sim.set_location_scenario(LocationScenario::MessagesOnly);
    sim.advance(REFRESH_MS);
    assert_eq!(sim.location().view().status, FixStatus::NoData);
    assert_eq!(
        sim.location().view().receiver_status,
        ReceiverStatus::Active
    );
    sim.set_location_scenario(LocationScenario::Silent);
    let messages = sim.snapshot().receiver_messages;
    sim.advance(5_000);
    assert_eq!(sim.location().view().receiver_status, ReceiverStatus::Quiet);
    assert_eq!(sim.snapshot().receiver_age_seconds, Some(5));
    assert_eq!(sim.snapshot().receiver_messages, messages);
    assert!(sim.location().view().satellites.is_none());
    sim.set_location_scenario(LocationScenario::MessagesOnly);
    assert_eq!(
        sim.location().view().receiver_status,
        ReceiverStatus::Active
    );
}

#[test]
fn diagnostic_reports_do_not_refresh_a_closed_screen() {
    let mut sim = Simulation::default();
    sim.set_location_scenario(LocationScenario::Acquiring);
    sim.advance(REFRESH_MS);
    let refreshes = sim.snapshot().completed_refreshes;
    sim.advance(20_000);
    assert_eq!(sim.snapshot().completed_refreshes, refreshes);
    assert!(sim.snapshot().receiver_messages > 10);
}

#[test]
fn elevation_can_be_negative_or_missing_without_losing_the_position() {
    let mut sim = Simulation::default();
    sim.set_location_scenario(LocationScenario::BelowSeaLevel);
    press(&mut sim, LOCATION_TARGET);
    sim.advance(2 * REFRESH_MS);
    assert_eq!(sim.snapshot().elevation_mm, Some(-430_500));
    assert_eq!(sim.location().view().status, FixStatus::Valid);
    let with_elevation = sim.display().pixels().to_vec();
    sim.set_location_scenario(LocationScenario::MissingElevation);
    assert_eq!(sim.snapshot().elevation_mm, None);
    assert_eq!(sim.location().view().status, FixStatus::Valid);
    assert!(sim.location().view().coordinates.is_some());
    sim.advance(REFRESH_MS);
    assert_ne!(sim.display().pixels(), with_elevation);
}

#[test]
fn motion_and_utc_support_movement_stops_and_independent_missing_fields() {
    let mut sim = Simulation::default();
    sim.set_location_scenario(LocationScenario::ZeroCoordinates);
    press(&mut sim, LOCATION_TARGET);
    assert_eq!(sim.snapshot().speed_mm_s, Some(2_780));
    assert_eq!(sim.snapshot().course_millidegrees, Some(315_000));
    assert_eq!(sim.snapshot().utc_time, Some([0, 0, 0]));
    sim.advance(5_000 + REFRESH_MS);
    assert_eq!(sim.snapshot().utc_time, Some([0, 0, 5]));
    let moving = sim.display().pixels().to_vec();
    sim.set_location_scenario(LocationScenario::Stopped);
    assert_eq!(sim.snapshot().speed_mm_s, Some(0));
    assert_eq!(sim.snapshot().course_millidegrees, None);
    sim.advance(REFRESH_MS);
    assert_ne!(sim.display().pixels(), moving);
    sim.set_location_scenario(LocationScenario::MissingMotion);
    assert_eq!(sim.snapshot().speed_mm_s, None);
    assert!(sim.snapshot().utc_time.is_some());
    assert!(sim.snapshot().latitude_microdegrees.is_some());
    sim.set_location_scenario(LocationScenario::MissingUtc);
    assert!(sim.snapshot().speed_mm_s.is_some());
    assert!(sim.snapshot().course_millidegrees.is_some());
    assert_eq!(sim.snapshot().utc_time, None);
    assert!(sim.snapshot().latitude_microdegrees.is_some());
}

#[test]
fn motion_and_utc_expire_independently_while_position_and_receiver_stay_current() {
    for scenario in [LocationScenario::StaleMotion, LocationScenario::StaleUtc] {
        let mut sim = Simulation::default();
        sim.set_location_scenario(scenario);
        press(&mut sim, LOCATION_TARGET);
        sim.advance(4_999);
        assert!(sim.snapshot().speed_mm_s.is_some());
        assert!(sim.snapshot().utc_time.is_some());
        sim.advance(1);
        assert_eq!(
            sim.snapshot().speed_mm_s.is_none(),
            scenario == LocationScenario::StaleMotion
        );
        assert_eq!(
            sim.snapshot().course_millidegrees.is_none(),
            scenario == LocationScenario::StaleMotion
        );
        assert_eq!(
            sim.snapshot().utc_time.is_none(),
            scenario == LocationScenario::StaleUtc
        );
        assert!(sim.snapshot().latitude_microdegrees.is_some());
        assert_eq!(
            sim.location().view().receiver_status,
            ReceiverStatus::Active
        );
    }
}

#[test]
fn lost_fix_silence_and_faults_remove_motion_and_utc() {
    let mut sim = Simulation::default();
    press(&mut sim, LOCATION_TARGET);
    for scenario in [LocationScenario::LostFix, LocationScenario::Silent] {
        sim.set_location_scenario(scenario);
        assert!(sim.snapshot().speed_mm_s.is_some());
        assert!(sim.snapshot().utc_time.is_some());
        sim.advance(if scenario == LocationScenario::LostFix {
            ACQUISITION_MS
        } else {
            5_000
        });
        assert_eq!(sim.snapshot().speed_mm_s, None);
        assert_eq!(sim.snapshot().course_millidegrees, None);
        assert_eq!(sim.snapshot().utc_time, None);
    }
    for scenario in [LocationScenario::Unavailable, LocationScenario::Fault] {
        sim.set_location_scenario(LocationScenario::ZeroCoordinates);
        assert!(sim.snapshot().utc_time.is_some());
        sim.set_location_scenario(scenario);
        assert_eq!(sim.snapshot().speed_mm_s, None);
        assert_eq!(sim.snapshot().course_millidegrees, None);
        assert_eq!(sim.snapshot().utc_time, None);
    }
}
