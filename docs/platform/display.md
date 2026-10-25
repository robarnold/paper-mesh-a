# Display lifecycle and ownership

[Platform overview](../platform.md)

## Planned display policy

Future component behavior and display interfaces use the
[display contract](../display-constraints.md).
This contract covers damage, quality intent, capabilities, completion, and
recovery. Backend cost estimates can inform scheduling without scan algorithms
or panel identifiers in shared policy.

A planned platform compositor and display policy will combine component damage
through the [orientation snapshot](orientation.md) before submission.

A physical panel can change visibly throughout a refresh.
A presented snapshot identifies the last completed target.
It does not indicate an atomic optical replacement.

## Current interface

The current implementation uses a direct `App` → `Display` path.

Drawing changes staging storage.
A `refresh()` result of `Ok(())` confirms acceptance of the frame.
It does not confirm panel completion.
While `is_busy()` is true, the application submits no drawing operations or
more refreshes. Input and timers can update application state.

The [application](../application.md#deadlines-and-display-submission) specifies its redraw state and retry behavior.
The platform controls driver recovery and retry delays.

The `refresh_full()` method requests a complete refresh, including unchanged pixels, without a partial waveform.
Backends with one refresh mode can use its default implementation.
The orientation adapter forwards the request to the native display.
The `present_full()` helper applies the same readiness, submission, and error rules as `present()`.
The [Diagnostics screen](../application.md#diagnostics) uses this contract.
The runtime supplies its completion event only after it regains an idle display without an error.

The current interface has no notification for asynchronous failure after acceptance.
Backend supervision controls these failures.
The [backend profiles](../hardware.md) specify the available recovery behavior.

## Buffer ownership

Native drivers own all buffers.
They keep transfer data valid until completion, including work through DMA or
another core. `Gray4` specifies 16 logical gray levels.
It does not specify byte layout or allocation strategy.

The [simulator execution](../simulator/execution.md) specifies its storage and frame states.
Hardware drivers can use different buffer strategies.
Each backend must obey the submission and busy-state contract.
