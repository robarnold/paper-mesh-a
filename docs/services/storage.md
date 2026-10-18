# Files and internal persistence

[Device services](../device-interfaces.md)

These services stay planned work.

Removable files and internal settings are separate services.
Media removal must not erase settings or prevent application startup.
The backend controls flash partitions, erase geometry, wear management, and
update slots. Volatile memory cannot supply durable settings storage.

`Files` reports media states such as unknown, absent, probing, mounted,
unmounted, read-only, and faulted.
It reports capacity and free space when known.
Presence detection can be immediate or delayed.
Applications use service observations.
Only the backend knows whether detection uses a signal, polling, or an I/O
failure. Simulated media changes must use the declared detection path.

Each mount has a media generation.
Replacement invalidates old handles and completions, even when filenames match.
Reads and writes must bound work and report partial progress.
Errors include full media, unsupported or corrupt filesystems, timeout, removal,
and I/O failure.

Write, sync, close, and unmount must have their own durability guarantees.
Sudden power or media loss can damage filesystem state.
Atomic replacement guarantees depend on a selected and validated storage strategy.

Resource contention must not prevent other services from progress.
Applications can operate without identification of the shared bus.
Unmount and electrical power removal are separate capabilities.
Neither operation implies the other.

`Settings` must recover the last or new committed version after interruption.
It must include schema handling, version handling, capacity results, and explicit errors.
The implementation must meet this guarantee before it reports the capability.
A simulator can keep durable media across an application restart.
It can discard volatile buffers and incomplete writes during that restart.
