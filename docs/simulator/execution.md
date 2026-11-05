# Simulator execution

[Simulator overview](../simulator.md)

## Display geometry

Use the portrait frame to review the layout as the user sees it.
Use the native frame to investigate rotation errors. An asymmetric image distinguishes a rotation error from a rendering error.
Drawing and touch use the same [orientation contract](../platform/orientation.md).

## Virtual time and display

Interactive sessions use wall time to pace a virtual clock. Pause a session to control when the next deadline occurs.
The simulator processes deadlines in order, so one large advance and several smaller advances reach the same state at the same virtual time.
This lets tests reproduce a change during a refresh without relying on host scheduling.

The previous frame stays visible until the simulated refresh completes.
Compare the application state with the visible frame to check how the application handles input while the display is busy.
The simulator replaces the frame in one step. Panel waveforms, ghosting, and optical settling require physical measurements.
See the [display contract](../display-constraints.md) and [executable timing checks](../../simulator/src/lib.rs).

## Snapshot commands

Run `just simulator-snapshot` in the devcontainer to capture both orientations without starting the server.
The [snapshot recipe](../../justfile) specifies the output paths.

## UI guide

The Retained layout section includes the production calculator in an independent session.
It demonstrates separate layout, graphics, and input properties with keyboard input and a reset shortcut on the focused preview.
The guide completes presentation synchronously. The device simulator retains its delayed display behavior.

Use the [interactive guide](http://localhost:8080/ui-guide) to try platform controls and inspect the Rust examples that draw them.
Each example has its own state. Browser tabs on the same server share that state, so an action in one tab can affect another.
Guide previews update immediately. Use the device simulator to check behavior during a pending refresh.
Follow the [shared guide rules](../ui.md#display-updates-and-validation) for previews, controls, and external state readouts.

For live refresh, run `devcontainer exec --workspace-folder . just sim-watch`.
The previous server stays available during a build and after a build failure.
A successful build restarts the server. Guide tabs reload at their current section, but example and device state reset.

## Visual catalog

Use the [catalog renderer](../../simulator/examples/ui_catalog.rs) to save a static reference for changes to the [shared visual language](../ui.md#visual-language).
The [diagnostics renderer](../../simulator/examples/diagnostics.rs) captures application pages and test patterns for comparison.
Run either example in the devcontainer, for example with `cargo run --locked -p paper-mesh-simulator --example ui_catalog`.
