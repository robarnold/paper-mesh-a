# Power observation ownership

The [portable observer](../../crates/power-observation/src/lib.rs) consumes platform battery and charger capabilities.
It has no knowledge of I2C, chip models, ESP peripherals, or board power control.

Each sample awaits the battery read and then the charger read. A failure does not skip the other source.
Successful observations receive separate completion timestamps from the supplied monotonic clock.
A failed source publishes `Fault` with no retained value. See the [power contract](../services/power.md).
Direct reads return a snapshot without publication or logging.

The worker publishes each complete sample and logs the first failure in each uninterrupted series of failures.
A successful read permits a later failure to produce another log message.
The runtime supplies the polling delay, so the observer does not choose polling policy or require a timer driver.

## Coordination and runtime ownership

The worker checks its pause handle before each sample. It completes both device reads and publication before acknowledging a pause.
A pause can interrupt the polling delay. It cannot cancel a read already in progress.
Dropping the worker future does not prove device quiescence. Use the pause acknowledgement before hardware transitions.

`Service` owns the cache, notification signal, and pause storage. A shared service reference implements platform `Power`.
Preparation returns `(Lifecycle, Workers)`. Callers retain their existing service reference. `Workers` implements platform `WorkerSet`.
The lifecycle handle implements suspend and power-off preparation through the service pause channel.
The service uses `CriticalSectionRawMutex` internally for cache, notification, and pause state.
Firmware supplies the platform critical-section implementation; native tests use its standard-library implementation.
The crate owns the Embassy task. Firmware supplies its spawner through `Execution`.
Preparation boxes the future once so device types do not require a firmware task declaration.
Polling uses dynamic dispatch but adds no allocation per sample. The task pool permits one active observation worker.

[Firmware integration](../../firmware/src/power/observation.rs) constructs the chip drivers with the shared bus.
It supplies static service storage and timing policy. The crate implements publication, notifications, and lifecycle behavior.
Direct suspend observations use `Service::read_now` while the periodic worker is paused.
That method samples and publishes to the same cache and change signal. Callers must coordinate worker quiescence.
Power removal and charger configuration remain separate runtime responsibilities.

## Verification limits

Native tests cover independent failures, recovery, completion timestamps, initial pause, and preparation during an incomplete read.
They verify publication before pause acknowledgement and new acquisition after resume.
Service tests exercise matching handles, initial cached state, notifications, direct publication, suspend cancellation, and power-off preparation.
Firmware checks compile concrete chip drivers and task storage. They do not establish physical power behavior or sampling latency.
