//! Ordered power control for the H752-01 panel and TPS65185 supplies.

pub const EXPANDER: u8 = 0x20;
pub const SUPPLY: u8 = 0x68;
pub const OE: u8 = 1;
pub const MODE: u8 = 2;
pub const PWRUP: u8 = 8;
pub const VCOM: u8 = 16;
pub const WAKE: u8 = 32;
pub const DISPLAY_OUTPUTS: u8 = OE | MODE | PWRUP | VCOM | WAKE;
pub const PG: u8 = 64;

// These guards supplement register checks. They do not measure analog voltages.
pub const DISCHARGE_MS: u32 = 600;
pub const SLEEP_MS: u32 = 150;
pub const UNKNOWN_DISCHARGE_MS: u32 = 4_000;
const LOGIC_SETTLE_MS: u32 = 10;
const VCOM_SETTLE_MS: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    LogicReady,
    RailsRequested,
    RailsReady,
    HighVoltageOff,
    WakeLow,
}

pub trait Access {
    type Error;
    fn read(&mut self, device: u8, register: u8) -> Result<u8, Self::Error>;
    fn write(&mut self, device: u8, register: u8, value: u8) -> Result<(), Self::Error>;
    fn wait_ms(&mut self, milliseconds: u32);
    fn now_ms(&self) -> u64;
    fn mark(&mut self, _stage: Stage) {}
}

#[derive(Debug, PartialEq)]
pub enum Error<E> {
    Transport(E),
    Verification(&'static str),
    Timeout(&'static str),
    Temperature(i8),
    SupplyFault(u8, u8),
    Faulted,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading {
    pub revision: u8,
    pub temperature: i8,
    pub vcom1: u8,
    pub vcom2: u8,
}

/// Values from existing PMIC reads during the most recent power sequence.
/// Missing values were not read or did not pass a required readback check.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Measurement {
    pub attempt: u32,
    pub started_ms: u64,
    pub ready_ms: Option<u64>,
    pub shutdown_ms: Option<u64>,
    pub revision: Option<u8>,
    pub temperature: Option<i8>,
    pub enable_start: Option<u8>,
    pub vcom1: Option<u8>,
    pub vcom2: Option<u8>,
    pub sequence: [Option<u8>; 4],
    pub faults_before: [Option<u8>; 2],
    pub faults_active: [Option<u8>; 2],
    pub power_good_start: Option<u8>,
    pub power_good_active: Option<u8>,
    pub power_good_off: Option<u8>,
    pub enable_off: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlSnapshot {
    pub input: u8,
    pub output: u8,
    pub inversion: u8,
    pub configuration: u8,
}

impl ControlSnapshot {
    pub fn controls_off(self) -> bool {
        self.configuration & DISPLAY_OUTPUTS == 0
            && self.output & DISPLAY_OUTPUTS == 0
            && (self.input ^ self.inversion) & DISPLAY_OUTPUTS == 0
    }
}

/// Read only port 1. These reads do not change the display power state.
pub fn inspect_controls<T: Access>(io: &mut T) -> Result<ControlSnapshot, Error<T::Error>> {
    let output = read(io, EXPANDER, 3)?;
    let configuration = read(io, EXPANDER, 7)?;
    let inversion = read(io, EXPANDER, 5)?;
    let input = read(io, EXPANDER, 1)?;
    Ok(ControlSnapshot {
        input,
        output,
        inversion,
        configuration,
    })
}

pub struct State {
    control: u8,
    faulted: bool,
    discharged: bool,
    measurement: Option<Measurement>,
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    pub const fn new() -> Self {
        Self {
            control: 0,
            faulted: false,
            discharged: true,
            measurement: None,
        }
    }

    pub fn control(&self) -> u8 {
        self.control
    }

    pub fn faulted(&self) -> bool {
        self.faulted
    }

    pub fn discharged(&self) -> bool {
        self.discharged
    }

    pub fn measurement(&self) -> Option<Measurement> {
        self.measurement
    }

    pub fn latch_fault(&mut self) {
        self.faulted = true;
        self.discharged = false;
    }

    pub fn blank_retained_outputs<T: Access>(&mut self, io: &mut T) -> Result<(), Error<T::Error>> {
        self.control = read(io, EXPANDER, 3)?;
        self.set_control(io, OE | MODE, 0)
    }

    pub fn set_control<T: Access>(
        &mut self,
        io: &mut T,
        mask: u8,
        value: u8,
    ) -> Result<(), Error<T::Error>> {
        let next = (self.control & !mask) | (value & mask);
        write_checked(io, EXPANDER, 3, next, 0xff, "expander output latch")?;
        // A failed write or readback must not change the last confirmed cache.
        self.control = next;
        Ok(())
    }

    /// Clear retained controls before any allocation or scan driver setup.
    pub fn establish_off<T: Access>(&mut self, io: &mut T) -> Result<(), Error<T::Error>> {
        // Port 0 remains untouched. The other port 1 bits are input pins.
        if let Ok(value) = io.read(EXPANDER, 3) {
            self.control = value;
        }
        let clear = self.set_control(io, DISPLAY_OUTPUTS, 0);
        let configuration = (|| {
            if clear.is_err() {
                return Err(Error::Verification("initial off request"));
            }
            let direction = read(io, EXPANDER, 7)?;
            write_checked(
                io,
                EXPANDER,
                7,
                (direction & !DISPLAY_OUTPUTS) | 0xc4,
                0xff,
                "expander direction",
            )?;
            let inversion = read(io, EXPANDER, 5)?;
            write_checked(io, EXPANDER, 5, inversion & !PG, 0xff, "expander inversion")?;
            Ok(())
        })();
        // Configure the safe latch before output directions. Then allow discharge.
        io.wait_ms(UNKNOWN_DISCHARGE_MS);
        let verify = self.verify_off(io);
        let result = clear.and(configuration).and(verify);
        if result.is_err() {
            // Retry the complete off value, even after a failed configuration read.
            let _ = self.set_control(io, DISPLAY_OUTPUTS, 0);
            io.wait_ms(UNKNOWN_DISCHARGE_MS);
            self.latch_fault();
        }
        result
    }

    pub fn verify_off<T: Access>(&self, io: &mut T) -> Result<(), Error<T::Error>> {
        self.verify_controls(io, 0)
    }

    fn verify_controls<T: Access>(&self, io: &mut T, expected: u8) -> Result<(), Error<T::Error>> {
        let snapshot = inspect_controls(io)?;
        if snapshot.configuration & DISPLAY_OUTPUTS != 0
            || snapshot.output & DISPLAY_OUTPUTS != expected
            || self.control() & DISPLAY_OUTPUTS != expected
            || (snapshot.input ^ snapshot.inversion) & DISPLAY_OUTPUTS != expected
            || (expected == 0 && !snapshot.controls_off())
        {
            return Err(Error::Verification("power control levels"));
        }
        Ok(())
    }

    pub fn power_on<T: Access>(
        &mut self,
        io: &mut T,
        magnitude_mv: u16,
    ) -> Result<Reading, Error<T::Error>> {
        let attempt = self
            .measurement()
            .map_or(1, |last| last.attempt.wrapping_add(1));
        self.measurement = Some(Measurement {
            attempt,
            started_ms: io.now_ms(),
            ..Measurement::default()
        });
        if self.faulted {
            return Err(Error::Faulted);
        }
        self.verify_off(io)?;
        self.discharged = false;
        self.set_control(io, DISPLAY_OUTPUTS, WAKE)?;
        io.wait_ms(3);
        // Supply panel logic before the high-voltage sequence can start.
        let enable = write_checked_value(io, SUPPLY, 1, 0x20, 0xbf, "V3P3 enable")?;
        self.measurement.as_mut().unwrap().enable_start = Some(enable);
        io.wait_ms(LOGIC_SETTLE_MS);
        io.mark(Stage::LogicReady);

        let revision = read(io, SUPPLY, 0x10)?;
        self.measurement.as_mut().unwrap().revision = Some(revision);
        let previous = read(io, SUPPLY, 4)?;
        let vcom1 = (magnitude_mv / 10) as u8;
        // Preserve reserved bit 2. Never request acquisition or EEPROM programming.
        let vcom2 = (previous & 4) | (((magnitude_mv / 10) >> 8) as u8 & 1);
        let vcom2 = write_checked_value(io, SUPPLY, 4, vcom2, 0xff, "VCOM2")?;
        self.measurement.as_mut().unwrap().vcom2 = Some(vcom2);
        let vcom1 = write_checked_value(io, SUPPLY, 3, vcom1, 0xff, "VCOM1")?;
        self.measurement.as_mut().unwrap().vcom1 = Some(vcom1);
        for (index, (register, expected)) in [(9, 0xe4), (10, 0x55), (11, 0x1e), (12, 0xe0)]
            .into_iter()
            .enumerate()
        {
            let actual = read(io, SUPPLY, register)?;
            self.measurement.as_mut().unwrap().sequence[index] = Some(actual);
            if actual != expected {
                return Err(Error::Verification("PMIC sequence configuration"));
            }
        }
        // Clear old latched flags before activation. Capture new faults below.
        self.measurement.as_mut().unwrap().faults_before[0] = Some(read(io, SUPPLY, 7)?);
        self.measurement.as_mut().unwrap().faults_before[1] = Some(read(io, SUPPLY, 8)?);
        io.write(SUPPLY, 0x0d, 0x80).map_err(Error::Transport)?;
        wait_register(io, 0x0d, 0x20, 0x20, "temperature")?;
        let temperature = read(io, SUPPLY, 0)? as i8;
        self.measurement.as_mut().unwrap().temperature = Some(temperature);
        if !(0..=50).contains(&temperature) {
            return Err(Error::Temperature(temperature));
        }

        io.mark(Stage::RailsRequested);
        self.set_control(io, PWRUP, PWRUP)?;
        let power_good = wait_register(io, 0x0f, 0xfa, 0xfa, "TPS65185 rails")?;
        self.measurement.as_mut().unwrap().power_good_start = Some(power_good);
        // ED047TC1 requires at least 100 us from VGH regulation to VCOM activation.
        io.wait_ms(1);
        self.set_control(io, VCOM, VCOM)?;
        io.wait_ms(VCOM_SETTLE_MS);
        let faults1 = read(io, SUPPLY, 7)?;
        self.measurement.as_mut().unwrap().faults_active[0] = Some(faults1);
        let faults2 = read(io, SUPPLY, 8)?;
        self.measurement.as_mut().unwrap().faults_active[1] = Some(faults2);
        if faults1 & 0x7c != 0 || faults2 & 0xfe != 0 {
            return Err(Error::SupplyFault(faults1, faults2));
        }
        let power_good = read(io, SUPPLY, 0x0f)?;
        self.measurement.as_mut().unwrap().power_good_active = Some(power_good);
        if power_good & 0xfa != 0xfa {
            return Err(Error::Verification("rails after VCOM activation"));
        }
        io.mark(Stage::RailsReady);
        self.measurement.as_mut().unwrap().ready_ms = Some(io.now_ms());
        Ok(Reading {
            revision,
            temperature,
            vcom1,
            vcom2,
        })
    }

    pub fn stop_high_voltage<T: Access>(&mut self, io: &mut T) -> Result<(), Error<T::Error>> {
        let blank = self.set_control(io, OE | MODE, 0);
        // ED047TC1 requires at least 100 us between data cessation and VCOM removal.
        io.wait_ms(1);
        let disable = self.set_control(io, OE | MODE | VCOM | PWRUP, 0);
        if disable.is_ok() {
            io.mark(Stage::HighVoltageOff);
        }
        // Keep panel logic and WAKE active throughout the HV discharge interval.
        io.wait_ms(DISCHARGE_MS);
        let discharge = wait_register(io, 0x0f, 0xfa, 0, "PMIC discharge");
        if let (Some(measured), Ok(value)) = (self.measurement.as_mut(), &discharge) {
            measured.power_good_off = Some(*value);
        }
        let controls = self.verify_controls(io, WAKE);
        let result = blank.and(disable).and(discharge).and(controls);
        self.discharged = result.is_ok();
        if result.is_err() {
            self.latch_fault();
        }
        result
    }

    /// Call only after driver release and explicit low levels on every panel input.
    pub fn finish_off<T: Access>(&mut self, io: &mut T) -> Result<(), Error<T::Error>> {
        let logic = if self.discharged() {
            write_checked_value(io, SUPPLY, 1, 0, 0xbf, "V3P3 disable")
        } else {
            Err(Error::Verification("unconfirmed discharge"))
        };
        if let (Some(measured), Ok(value)) = (self.measurement.as_mut(), &logic) {
            measured.enable_off = Some(*value);
        }
        // Always request autonomous shutdown, including after a bus failure.
        let sleep = self.set_control(io, DISPLAY_OUTPUTS, 0);
        if sleep.is_ok() {
            io.mark(Stage::WakeLow);
        }
        io.wait_ms(if logic.is_ok() && sleep.is_ok() {
            SLEEP_MS
        } else {
            UNKNOWN_DISCHARGE_MS
        });
        let off = self.verify_off(io);
        let result = logic.and(sleep).and(off);
        if result.is_err() {
            self.latch_fault();
        } else if let Some(measured) = self.measurement.as_mut() {
            measured.shutdown_ms = Some(io.now_ms());
        }
        result
    }
}

#[cfg(test)]
#[path = "power_sequence/tests.rs"]
mod tests;

fn read<T: Access>(io: &mut T, device: u8, register: u8) -> Result<u8, Error<T::Error>> {
    io.read(device, register).map_err(Error::Transport)
}

fn write_checked<T: Access>(
    io: &mut T,
    device: u8,
    register: u8,
    value: u8,
    mask: u8,
    stage: &'static str,
) -> Result<(), Error<T::Error>> {
    write_checked_value(io, device, register, value, mask, stage).map(|_| ())
}

fn write_checked_value<T: Access>(
    io: &mut T,
    device: u8,
    register: u8,
    value: u8,
    mask: u8,
    stage: &'static str,
) -> Result<u8, Error<T::Error>> {
    io.write(device, register, value)
        .map_err(Error::Transport)?;
    let actual = read(io, device, register)?;
    if actual & mask != value & mask {
        return Err(Error::Verification(stage));
    }
    Ok(actual)
}

fn wait_register<T: Access>(
    io: &mut T,
    register: u8,
    mask: u8,
    value: u8,
    stage: &'static str,
) -> Result<u8, Error<T::Error>> {
    let deadline = io.now_ms().saturating_add(500);
    loop {
        let actual = read(io, SUPPLY, register)?;
        if actual & mask == value {
            return Ok(actual);
        }
        if io.now_ms() >= deadline {
            return Err(Error::Timeout(stage));
        }
        io.wait_ms(1);
    }
}
