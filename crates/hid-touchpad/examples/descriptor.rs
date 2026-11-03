//! Export the touchpad report descriptor for host inspection.
//!
//! # Design choices
//!
//! - Export the HID descriptor for inspection from the same definition used to
//!   describe the device to a host.
//!
//! See [HID touchpad protocol](../README.md).

use paper_mesh_hid_touchpad::{Descriptor, Geometry};
fn main() {
    let descriptor = Descriptor::new(Geometry {
        width: 540,
        height: 960,
        width_tenth_mm: 583,
        height_tenth_mm: 1037,
    });
    std::io::Write::write_all(&mut std::io::stdout(), descriptor.as_bytes()).unwrap();
}
