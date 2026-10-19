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
