//! Export the same visual catalog that the browser guide displays.
//!
//! # Design choices
//!
//! - Use the guide renderer for catalog exports so a saved reference shows the same
//!   controls as the browser.
//!
//! See [Visual catalog](../../docs/simulator/execution.md#visual-catalog).

use paper_mesh_simulator::ui_guide::{Sheet, catalog_png};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/ui-catalog.png".into());
    std::fs::write(output, catalog_png(Sheet::Overview)?)?;
    Ok(())
}
