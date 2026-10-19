use embedded_graphics::prelude::Point;
use paper_mesh_platform::{TouchEvent, touchpad::HostState};
use paper_mesh_simulator::{REFRESH_MS, Simulation};

fn open(sim: &mut Simulation) {
    sim.advance(REFRESH_MS);
    let point = Point::new(200, 730);
    sim.touch_portrait(TouchEvent::touch(0, point));
    sim.touch_portrait(TouchEvent::release(0, point));
    sim.advance(REFRESH_MS);
    assert!(sim.snapshot().touchpad_open);
    assert_eq!(sim.snapshot().touchpad_reports, 0);
}

#[test]
fn mode_forwards_raw_contacts_without_refreshing_the_display() {
    let mut sim = Simulation::default();
    open(&mut sim);
    let refreshes = sim.snapshot().completed_refreshes;
    sim.touch_portrait(TouchEvent::touch(2, Point::new(100, 200)));
    sim.touch_portrait(TouchEvent::touch(7, Point::new(200, 300)));
    sim.touch_portrait(TouchEvent::touch(2, Point::new(120, 220)));
    assert_eq!(sim.snapshot().contacts.len(), 2);
    assert_eq!(sim.snapshot().contacts[0].id, 2);
    assert_eq!(sim.snapshot().contacts[0].x, 120);
    assert_eq!(sim.snapshot().touchpad_reports, 3);
    sim.advance(30_000);
    assert_eq!(sim.snapshot().completed_refreshes, refreshes);
    sim.touch_portrait(TouchEvent::release(2, Point::new(120, 220)));
    assert_eq!(sim.snapshot().contacts.len(), 1);
    assert_eq!(sim.snapshot().contacts[0].id, 7);
}

#[test]
fn function_and_home_exit_and_suppress_a_held_contact() {
    for button in [1, 2] {
        let mut sim = Simulation::default();
        open(&mut sim);
        sim.touch_portrait(TouchEvent::touch(3, Point::new(200, 730)));
        sim.hardware_button(button, true).unwrap();
        assert!(!sim.snapshot().touchpad_open);
        assert_eq!(sim.snapshot().touchpad_detachments, 1);
        sim.touch_portrait(TouchEvent::release(3, Point::new(200, 730)));
        assert!(!sim.snapshot().touchpad_open);
        assert_eq!(sim.snapshot().touchpad_reports, 1);
        sim.hardware_button(button, false).unwrap();
        sim.touch_portrait(TouchEvent::touch(3, Point::new(200, 730)));
        sim.touch_portrait(TouchEvent::release(3, Point::new(200, 730)));
        assert!(sim.snapshot().touchpad_open);
    }
}

#[test]
fn reconnect_and_cancel_clear_host_state() {
    let mut sim = Simulation::default();
    open(&mut sim);
    sim.touch_portrait(TouchEvent::touch(4, Point::new(100, 200)));
    sim.set_touchpad_host(HostState::Suspended);
    sim.set_touchpad_host(HostState::Ready);
    sim.touch_portrait(TouchEvent::touch(4, Point::new(120, 220)));
    assert_eq!(sim.snapshot().touchpad_reports, 1);
    sim.touch_portrait(TouchEvent::release(4, Point::new(120, 220)));
    sim.touch_portrait(TouchEvent::touch(4, Point::new(120, 220)));
    assert_eq!(sim.snapshot().touchpad_reports, 2);
    sim.touch_portrait(TouchEvent::Cancel);
    assert!(sim.snapshot().contacts.is_empty());
    assert_eq!(sim.snapshot().touchpad_detachments, 3);
}
