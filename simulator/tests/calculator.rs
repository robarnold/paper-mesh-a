//! Calculator navigation and asynchronous presentation checks.
//!
//! # Design choices
//!
//! - Drive the shared touch path so busy frames exercise retained input and damage.
//!
//! See [calculator behavior](../../docs/application.md#calculator).

use embedded_graphics::prelude::Point;
use paper_mesh_platform::TouchEvent;
use paper_mesh_simulator::{REFRESH_MS, Simulation};

fn tap(sim: &mut Simulation, label: &str) {
    let c = sim
        .snapshot()
        .controls
        .into_iter()
        .find(|c| c.label == label)
        .unwrap();
    let p = Point::new(c.x, c.y);
    sim.touch_portrait(TouchEvent::touch(0, p));
    sim.touch_portrait(TouchEvent::release(0, p));
}

// Rapid edits combine while busy, then the completed frame matches a fresh full-page render.
#[test]
fn busy_edits_repaint_only_the_readout_and_survive_navigation() {
    let mut sim = Simulation::default();
    sim.advance(REFRESH_MS);
    tap(&mut sim, "Calculator");
    sim.advance(REFRESH_MS);
    let before = sim.display().pixels().to_vec();
    for label in ["2", "+", "3", "x", "4", "="] {
        tap(&mut sim, label);
    }
    assert_eq!(sim.snapshot().calculator_value, "20");
    sim.advance(2 * REFRESH_MS);
    let incremental = sim.display().pixels().to_vec();
    assert_ne!(before, incremental);
    // Portrait y >= 256 is the keypad. Its native x coordinate is portrait y.
    for y in 0..540 {
        for x in 256..960 {
            assert_eq!(before[y * 960 + x], incremental[y * 960 + x]);
        }
    }
    tap(&mut sim, "Back");
    sim.advance(REFRESH_MS);
    assert!(!sim.snapshot().calculator_open);
    tap(&mut sim, "Calculator");
    sim.advance(REFRESH_MS);
    assert_eq!(sim.snapshot().calculator_value, "20");
    assert_eq!(sim.display().pixels(), incremental);
    let count = sim.snapshot().completed_refreshes;
    tap(&mut sim, "=");
    sim.advance(REFRESH_MS);
    assert_eq!(sim.snapshot().completed_refreshes, count);
    sim.advance(30_000);
    assert_eq!(sim.snapshot().completed_refreshes, count);
}

// Committed keys accept input before presentation, and Home retains calculations.
#[test]
fn latest_committed_keys_work_before_presentation() {
    let mut sim = Simulation::default();
    sim.advance(REFRESH_MS);
    tap(&mut sim, "Calculator");
    tap(&mut sim, "7");
    assert_eq!(sim.snapshot().calculator_value, "7");
    sim.advance(REFRESH_MS);
    tap(&mut sim, "8");
    sim.hardware_button(2, true).unwrap();
    sim.hardware_button(2, false).unwrap();
    sim.advance(2 * REFRESH_MS);
    assert!(!sim.snapshot().calculator_open);
    tap(&mut sim, "Calculator");
    sim.advance(REFRESH_MS);
    assert_eq!(sim.snapshot().calculator_value, "78");
    assert_eq!(Simulation::default().snapshot().calculator_value, "0");
}
