use paper_mesh_simulator::Simulation;

#[test]
fn hardware_controls_keep_identity_and_reject_lifecycle_input() {
    let mut sim = Simulation::default();
    sim.hardware_button(1, true).unwrap();
    assert!(sim.snapshot().hardware_buttons[1].pressed);
    assert!(!sim.snapshot().hardware_buttons[0].pressed);
    sim.hardware_button(1, false).unwrap();
    assert!(!sim.snapshot().hardware_buttons[1].pressed);
    assert!(sim.hardware_button(3, true).is_err());
    assert!(sim.hardware_button(4, true).is_err());
    assert!(sim.hardware_button(255, true).is_err());
    assert_eq!(sim.snapshot().taps, 0);
}
