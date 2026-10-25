# Simulator execution

[Simulator overview](../simulator.md)

## Display geometry

The current demonstration uses a 540 × 960 portrait coordinate space.
Staging, pending, and visible framebuffers use native 960 × 540 row-major storage.
Each buffer uses one byte per pixel.
Drawing and touch use the shared portrait rotation adapter.
The browser displays a portrait view of the completed native frame.
This rotation is fixed.

The [orientation design](../platform/orientation.md)
gives information about planned mounting configuration, automatic orientation, manual locks,
and rotation state associated with frames.

## Virtual time and display

`Simulation::advance` visits application and service deadlines in time order.
One 90-second advance gives the same observable result as many small advances.
Tests do not sleep or depend on wall-clock speed.
Interactive mode uses wall time to pace the same virtual clock.
Pause disables this pacing.
Reset keeps the current paused or active setting.

The display starts white.
Refresh has an artificial delay of 250 ms.
The last image stays visible until completion replaces it.
This atomic replacement simplifies simulation.
Physical e-paper changes throughout its drive sequence.

[Display constraints](../display-constraints.md) give information about planned damage costs,
waveform phases, and region scheduling.
Pixels have 16 grayscale levels.
PNG export keeps these values in an 8-bit grayscale image.
Idle polling does not refresh the screen.
Touch changes and heartbeat updates cause refreshes.

Tests cover input cancellation, bounds, heartbeat intervals, and frame visibility.
They also cover changes during refresh, failure and retry, equivalent time
advances, and PNG output.
Rotation tests check asymmetric native pixel layouts.
They compare the completed portrait view with independent direct rendering.
`just ci` produces `target/simulator/hello.png` and `target/simulator/native.png`
for visual checks of both orientations.

## Snapshot commands

Run these commands from the repository root on the host:

```sh
# Run native Clippy and deterministic application and device scenarios.
devcontainer exec --workspace-folder . just native-check

# Render a screenshot without a web server or display.
devcontainer exec --workspace-folder . just simulator-snapshot
```

The snapshot command writes `target/simulator/hello.png` for the portrait view.
It also writes `target/simulator/native.png` for the native framebuffer.
During simulation, [/frame-native.png](http://localhost:8080/frame-native.png) supplies the native image.

Native tests compare packed hardware pixels with simulator frames.
They exercise initial output, heartbeat updates, and partial counter updates.
These comparisons do not verify optical output or touch calibration.

## UI guide

With the simulator running, open [localhost:8080/ui-guide](http://localhost:8080/ui-guide).
The simulator home page also links to this guide.
Its sections cover Shell and Page, text and spacing, and controls.
Every section has an interactive device preview. Tap its controls directly without changing guide sections.
Each preview uses the platform renderer and input path.
Expandable code examples come directly from the compiled example source.
Reference links expose the shared standard, application rules, and toolkit API as Markdown.

The Shell and Page example launches one shell with retained Home, Tools, and Detail pages through the platform navigator.
Open Detail, increase its count, return Home, and open Detail again to inspect retained state.
Open Tools, then Detail. Back now returns to Tools because Tools invoked that page.
Home clears the return history without resetting the retained pages.
The pill above the preview shows all return destinations followed by the current page.
For example, **Home → Tools → Detail** identifies Tools as the next Back destination.
Home and Reset example return the pill to **Home**.
Try full-screen presentation and the simulated hardware Home key.
Start the example operation to block navigation. Select **Finish operation** to release the restriction.
Entry and exit counters show the lifecycle callbacks.
Expand **Keyboard controls** below each preview for equivalent browser buttons.

The Controls section has a separate interactive preview.
Tap the device screen to increase the action count, change the selection, or start an example operation.
During the operation, select **Finish operation** on the device screen to enable the other controls.
The destination card opens a detail page within the same preview. **Back** returns to the controls.
These actions keep the Controls tab active.
The Text and spacing preview has controls for text roles, underline, bubbles, spacing, and color roles.
Select a text role to change the sample font and color.
Select **Underline** to compare normal and underlined variants. The selection stays active when the role changes.
Select **Bubbled** to put the short labels and counts inside rounded badges. Underline can remain active inside a badge.
Select a gap to compare its value and the space between two blocks.
Select a color swatch to read its role and grayscale value.

The guide has its own state and performs no device operations.
Each interactive example has separate state and a separate reset action.
One guide instance exists per server. Browser tabs that use the guide share that instance.
Frames update immediately, without simulated refresh delay or physical display timing.
The example layouts and workflows reside in [`crates/app/examples/ui_guide/`](../../crates/app/examples/ui_guide/mod.rs).
The native host supplies input transport, PNG output, and the guide interface.
Firmware does not include these examples.

## Visual catalog

The catalog renders shared text roles, control states, destination cards, and grayscale roles through the platform Shell and Page APIs.
It uses the production components and the same catalog renderer as the browser guide.
This static reference is separate from the guide's interactive device previews.
Run these commands from the repository root on the host:

```sh
devcontainer exec --workspace-folder . cargo run --locked -p paper-mesh-simulator --example ui_catalog
devcontainer exec --workspace-folder . cargo run --locked -p paper-mesh-simulator --example diagnostics
```

The first command writes `target/ui-catalog.png`.
The second command writes application page images and solid patterns under `target/diagnostics/`.
Compare both sets after changes to the [shared visual language](../ui.md#visual-language).
