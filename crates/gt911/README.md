# GT911 reports

This `no_std` crate reads up to five contacts through a borrowed embedded-hal I2C bus.
It preserves controller configuration and uses controller coordinates.
Each ready coordinate report retains its contact ID and reaches the consumer.
The driver does not recognize taps. The platform supplies gesture recognition.
The firmware adapter owns reset timing, mounting transforms, and the event queue.

[Goodix Programming Guide Rev.10](https://www.lcd-module.de/fileadmin/eng/pdf/zubehoer/GT911_Programming_Guide_Rev.10.pdf),
sections 3.3 and 5, defines the registers and report sequence.
Tests cover report readiness, read-before-acknowledge order, release coordinates, read failures, and multiple contacts.
Each frame contains all contact updates and releases from one acknowledged report.
Report order does not change contact identity.
Malformed IDs, duplicate IDs, invalid coordinates, excess contacts, and palm reports cancel input.
After a fault, the driver waits for all contacts to lift.
