use embedded_graphics::prelude::Point;
use paper_mesh_platform::TouchEvent;
use paper_mesh_simulator::Simulation;

#[test]
fn movement_and_partial_releases_preserve_simulated_contact_ids() {
    let mut sim = Simulation::default();
    sim.touch_portrait(TouchEvent::touch(3, Point::new(100, 400)));
    sim.touch_portrait(TouchEvent::touch(12, Point::new(200, 400)));
    sim.touch_portrait(TouchEvent::touch(3, Point::new(110, 420)));
    let snapshot = sim.snapshot();
    assert_eq!(snapshot.contacts.len(), 2);
    assert_eq!(snapshot.contacts[0].id, 3);
    assert_eq!((snapshot.contacts[0].x, snapshot.contacts[0].y), (110, 420));
    sim.touch_portrait(TouchEvent::release(12, Point::new(200, 400)));
    assert_eq!(sim.snapshot().contacts.len(), 1);
    sim.touch_portrait(TouchEvent::release(3, Point::new(110, 420)));
    assert!(sim.snapshot().contacts.is_empty());
    assert_eq!(sim.snapshot().taps, 0);
}
