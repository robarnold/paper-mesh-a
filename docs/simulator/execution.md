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
