# Simulator scenarios

Use the scenario controls to supply observations to the same application that runs on the device.
Pause virtual time to inspect a transition before or after a deadline.
Scenario values and delays are test inputs. Physical measurements are necessary to establish device performance.

## Radio debugger

Select a scenario before starting a scan. The selection applies to the next request.
An empty result means the scan completed. A timeout means the application has no completed result to display.
See the [discovery fixtures](../../simulator/src/discovery.rs) and [scan timing checks](../../simulator/src/lib.rs).

## GNSS screen

Change the scenario while the location screen is open to start a new observation interval.
For example, a silent receiver lets the last fix expire. Continued receiver messages can keep activity current while position or satellite data expires.
Use the [location fixtures](../../simulator/src/location.rs) and [location tests](../../simulator/tests/location.rs) to reproduce these distinctions.

## Calendar clock

Virtual elapsed time advances the calendar after receiver loss.
Inject a UTC correction to check that a change to civil time leaves monotonic deadlines intact.
The [clock tests](../../simulator/tests/clock.rs) exercise this independently of the host clock.

## Power screen

A gauge fault can leave charger data available. Use independent source failures to check that the application still displays valid observations.
See the [power fixtures](../../simulator/src/power.rs) and [power tests](../../simulator/tests/power.rs).

Suspend and power off both wait for the final frame.
During suspend, a wake contact must clear before it can activate a control.
After power off, the frame stays visible until the simulator restarts.
These checks exercise application sequencing. Supply removal and sleep current require hardware measurements.
See the [suspend](../../simulator/tests/suspend.rs) and [power-off scenarios](../../simulator/tests/power_off.rs).

## Diagnostics

During the display check, try advancing the pattern before and after refresh completion.
Only the completed pattern can advance. This preserves each image for inspection.
The [diagnostics tests](../../simulator/tests/diagnostics.rs) exercise that boundary with the application renderer.

## USB touchpad

Hold Shift during a mouse click to retain a contact. Press Escape to cancel retained contacts.
Change the simulated host state with a contact held to check that reconnecting cannot replay an old press.
The [touchpad tests](../../simulator/tests/touchpad.rs) exercise report delivery and cancellation. USB packet timing requires a physical host.
