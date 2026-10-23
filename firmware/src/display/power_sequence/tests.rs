use super::{
    Access, DISCHARGE_MS, DISPLAY_OUTPUTS, EXPANDER, Error, MODE, OE, PWRUP, SLEEP_MS, SUPPLY,
    Stage, State, UNKNOWN_DISCHARGE_MS, VCOM, WAKE, inspect_controls,
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Event {
    Read(u8, u8),
    Write(u8, u8, u8),
    Wait(u32),
    Mark(Stage),
}

struct Fake {
    expander: [u8; 8],
    supply: [u8; 17],
    events: Vec<Event>,
    now: u64,
    calls: usize,
    fail: Option<usize>,
    fail_after_write: bool,
    corrupt: Option<usize>,
    stuck_pg: bool,
    activation_fault: u8,
    pg_extra: u8,
    enable_extra: u8,
}

impl Fake {
    fn new() -> Self {
        let mut supply = [0; 17];
        supply[0] = 25;
        supply[4] = 4;
        supply[9..13].copy_from_slice(&[0xe4, 0x55, 0x1e, 0xe0]);
        supply[16] = 0x65;
        let mut expander = [0; 8];
        expander[7] = 0xc4;
        Self {
            expander,
            supply,
            events: Vec::new(),
            now: 0,
            calls: 0,
            fail: None,
            fail_after_write: false,
            corrupt: None,
            stuck_pg: false,
            activation_fault: 0,
            pg_extra: 0,
            enable_extra: 0,
        }
    }

    fn failing(&mut self) -> bool {
        self.calls += 1;
        self.fail == Some(self.calls)
    }

    fn controls(&self) -> u8 {
        self.expander[3] & DISPLAY_OUTPUTS
    }

    fn assert_off(&self) {
        assert_eq!(self.controls(), 0);
    }

    fn shutdown(&mut self, state: &mut State) -> Result<(), Error<&'static str>> {
        let hv = state.stop_high_voltage(self);
        // The adapter releases drivers and makes GPIO low between these calls.
        let logic = state.finish_off(self);
        hv.and(logic)
    }
}

impl Access for Fake {
    type Error = &'static str;

    fn read(&mut self, device: u8, register: u8) -> Result<u8, Self::Error> {
        self.events.push(Event::Read(device, register));
        if self.failing() {
            return Err("injected read failure");
        }
        let value = match (device, register) {
            (EXPANDER, 1) => self.expander[3] ^ self.expander[5],
            (EXPANDER, reg) => self.expander[reg as usize],
            (SUPPLY, 0x0f) => {
                let base = if self.stuck_pg || self.controls() & (PWRUP | WAKE) == PWRUP | WAKE {
                    0xfa
                } else {
                    0
                };
                base | self.pg_extra
            }
            (SUPPLY, 1) => self.supply[1] | self.enable_extra,
            (SUPPLY, 8) if self.controls() & VCOM != 0 => self.activation_fault,
            (SUPPLY, reg) => self.supply[reg as usize],
            _ => panic!("unexpected device"),
        };
        Ok(if self.corrupt == Some(self.calls) {
            value ^ 1
        } else {
            value
        })
    }

    fn write(&mut self, device: u8, register: u8, value: u8) -> Result<(), Self::Error> {
        self.events.push(Event::Write(device, register, value));
        let fail = self.failing();
        if fail && !self.fail_after_write {
            return Err("injected write failure");
        }
        match (device, register) {
            (EXPANDER, 3) => {
                if value & PWRUP != 0 && self.controls() & PWRUP == 0 {
                    assert_eq!(self.supply[1] & 0xbf, 0x20, "HV preceded logic power");
                    assert_eq!(self.supply[3], 160, "HV preceded VCOM setup");
                    assert_eq!(self.supply[4], 4, "reserved VCOM2 bit lost");
                    assert_eq!(value & (OE | MODE | VCOM), 0);
                }
                self.expander[3] = value;
            }
            (EXPANDER, reg) => self.expander[reg as usize] = value,
            (SUPPLY, 0x0d) => self.supply[0x0d] = 0x20,
            (SUPPLY, reg) => self.supply[reg as usize] = value,
            _ => panic!("unexpected device"),
        }
        if fail {
            Err("ambiguous completed write")
        } else {
            Ok(())
        }
    }

    fn wait_ms(&mut self, milliseconds: u32) {
        self.now += u64::from(milliseconds);
        self.events.push(Event::Wait(milliseconds));
    }

    fn now_ms(&self) -> u64 {
        self.now
    }

    fn mark(&mut self, stage: Stage) {
        self.events.push(Event::Mark(stage));
    }
}

#[test]
fn power_order_retains_logic_through_discharge_and_separates_driver_release() {
    let mut io = Fake::new();
    let mut state = State::new();
    let reading = state.power_on(&mut io, 1600).unwrap();
    assert_eq!(
        (
            reading.revision,
            reading.temperature,
            reading.vcom1,
            reading.vcom2
        ),
        (0x65, 25, 160, 4)
    );
    assert_eq!(io.controls(), WAKE | PWRUP | VCOM);
    assert_eq!(state.control(), io.expander[3]);
    let enable = io
        .events
        .iter()
        .position(|event| *event == Event::Write(SUPPLY, 1, 0x20))
        .unwrap();
    let hv = io
        .events
        .iter()
        .position(|event| *event == Event::Write(EXPANDER, 3, WAKE | PWRUP))
        .unwrap();
    assert!(enable < hv);
    state.stop_high_voltage(&mut io).unwrap();
    assert_eq!(io.controls(), WAKE);
    assert_eq!(
        io.supply[1], 0x20,
        "VDD must survive the driver-release boundary"
    );
    assert!(io.events.contains(&Event::Wait(DISCHARGE_MS)));
    state.finish_off(&mut io).unwrap();
    assert_eq!(io.supply[1], 0);
    io.assert_off();
    assert!(io.events.contains(&Event::Wait(SLEEP_MS)));
    assert!(!state.faulted());
}

#[test]
fn every_startup_transport_failure_has_an_off_cleanup_path() {
    let mut baseline = Fake::new();
    State::new().power_on(&mut baseline, 1600).unwrap();
    for fail_at in 1..=baseline.calls {
        for after in [false, true] {
            let mut io = Fake::new();
            let mut state = State::new();
            io.fail = Some(fail_at);
            io.fail_after_write = after;
            assert!(
                state.power_on(&mut io, 1600).is_err(),
                "operation {fail_at}"
            );
            let _ = io.shutdown(&mut state);
            io.assert_off();
        }
    }
}

#[test]
fn every_shutdown_transport_failure_blocks_another_refresh() {
    let mut baseline = Fake::new();
    let mut state = State::new();
    state.power_on(&mut baseline, 1600).unwrap();
    baseline.calls = 0;
    baseline.shutdown(&mut state).unwrap();
    for fail_at in 1..=baseline.calls {
        for after in [false, true] {
            let mut io = Fake::new();
            let mut state = State::new();
            state.power_on(&mut io, 1600).unwrap();
            io.calls = 0;
            io.fail = Some(fail_at);
            io.fail_after_write = after;
            assert!(io.shutdown(&mut state).is_err(), "operation {fail_at}");
            assert!(state.faulted());
            let calls = io.calls;
            assert_eq!(state.power_on(&mut io, 1600), Err(Error::Faulted));
            assert_eq!(io.calls, calls, "faulted power-on must not touch hardware");
        }
    }
}

#[test]
fn cache_changes_only_after_a_successful_write_and_readback() {
    for (fail, corrupt, after) in [
        (Some(1), None, false),
        (Some(1), None, true),
        (Some(2), None, false),
        (None, Some(2), false),
    ] {
        let mut io = Fake::new();
        let mut state = State::new();
        io.fail = fail;
        io.corrupt = corrupt;
        io.fail_after_write = after;
        assert!(state.set_control(&mut io, DISPLAY_OUTPUTS, WAKE).is_err());
        assert_eq!(state.control(), 0);
    }
}

#[test]
fn retained_boot_latch_is_cleared_before_directions_and_waits_for_unknown_sequence() {
    let mut io = Fake::new();
    let mut state = State::new();
    io.expander[3] = 0xff;
    io.expander[7] = 0xff;
    state.blank_retained_outputs(&mut io).unwrap();
    assert_eq!(io.controls(), WAKE | PWRUP | VCOM);
    state.establish_off(&mut io).unwrap();
    io.assert_off();
    let clear = io
        .events
        .iter()
        .position(|event| *event == Event::Write(EXPANDER, 3, 0xc4))
        .unwrap();
    let direction = io
        .events
        .iter()
        .position(|event| *event == Event::Write(EXPANDER, 7, 0xc4))
        .unwrap();
    assert!(clear < direction);
    assert_eq!(io.now, u64::from(UNKNOWN_DISCHARGE_MS));
    state.verify_off(&mut io).unwrap();
}

#[test]
fn failed_boot_clear_does_not_enable_output_directions() {
    let mut io = Fake::new();
    let mut state = State::new();
    io.expander[3] = 0xff;
    io.expander[7] = 0xff;
    io.fail = Some(2);
    assert!(state.establish_off(&mut io).is_err());
    assert_eq!(io.expander[7], 0xff);
    assert!(state.faulted());
    io.assert_off();
}

#[test]
fn unknown_sequence_and_unsafe_temperature_never_enable_high_voltage() {
    for temperature in [25, -1, 51] {
        let mut io = Fake::new();
        let mut state = State::new();
        io.supply[0] = temperature as u8;
        if temperature == 25 {
            io.supply[12] = 0xff;
        }
        assert!(state.power_on(&mut io, 1600).is_err());
        assert_eq!(io.controls() & (PWRUP | VCOM | OE | MODE), 0);
        io.shutdown(&mut state).unwrap();
    }
}

#[test]
fn discharge_timeout_preserves_vdd_until_fallback_and_latches_fault() {
    let mut io = Fake::new();
    let mut state = State::new();
    state.power_on(&mut io, 1600).unwrap();
    io.stuck_pg = true;
    let start = io.now;
    assert_eq!(
        state.stop_high_voltage(&mut io),
        Err(Error::Timeout("PMIC discharge"))
    );
    assert_eq!(io.supply[1], 0x20);
    assert!(state.finish_off(&mut io).is_err());
    io.assert_off();
    assert!(state.faulted());
    assert_eq!(
        io.now - start,
        u64::from(1 + DISCHARGE_MS + 500 + UNKNOWN_DISCHARGE_MS)
    );
}

#[test]
fn vcom_fault_after_activation_prevents_scanning() {
    let mut io = Fake::new();
    let mut state = State::new();
    io.activation_fault = 4;
    assert_eq!(state.power_on(&mut io, 1600), Err(Error::SupplyFault(0, 4)));
    assert!(!io.events.contains(&Event::Mark(Stage::RailsReady)));
    assert_eq!(io.controls() & (OE | MODE), 0);
    io.shutdown(&mut state).unwrap();
    io.assert_off();
}

#[test]
fn mismatched_voltage_register_readbacks_prevent_high_voltage() {
    let mut baseline = Fake::new();
    State::new().power_on(&mut baseline, 1600).unwrap();
    for (register, occurrence) in [(1, 1), (4, 2), (3, 1)] {
        let mut calls = 0;
        let mut reads = 0;
        let mut corrupt = 0;
        for event in &baseline.events {
            if matches!(event, Event::Read(..) | Event::Write(..)) {
                calls += 1;
            }
            if *event == Event::Read(SUPPLY, register) {
                reads += 1;
                if reads == occurrence {
                    corrupt = calls;
                    break;
                }
            }
        }
        assert_ne!(corrupt, 0);
        let mut io = Fake::new();
        let mut state = State::new();
        io.corrupt = Some(corrupt);
        assert!(matches!(
            state.power_on(&mut io, 1600),
            Err(Error::Verification(_))
        ));
        assert_eq!(io.controls() & (PWRUP | VCOM | OE | MODE), 0);
        io.shutdown(&mut state).unwrap();
        io.assert_off();
    }
}

#[test]
fn final_off_verification_rejects_an_output_direction_or_physical_level_mismatch() {
    for register in [1, 3, 7] {
        let mut baseline = Fake::new();
        let mut state = State::new();
        state.power_on(&mut baseline, 1600).unwrap();
        state.stop_high_voltage(&mut baseline).unwrap();
        baseline.calls = 0;
        baseline.events.clear();
        state.finish_off(&mut baseline).unwrap();
        let mut calls = 0;
        let mut corrupt = 0;
        for event in &baseline.events {
            if matches!(event, Event::Read(..) | Event::Write(..)) {
                calls += 1;
            }
            if *event == Event::Read(EXPANDER, register) {
                corrupt = calls;
            }
        }
        let mut io = Fake::new();
        let mut state = State::new();
        state.power_on(&mut io, 1600).unwrap();
        state.stop_high_voltage(&mut io).unwrap();
        io.calls = 0;
        io.corrupt = Some(corrupt);
        assert!(state.finish_off(&mut io).is_err());
        assert!(state.faulted());
        assert_eq!(state.power_on(&mut io, 1600), Err(Error::Faulted));
    }
}

#[test]
fn retained_measurements_use_raw_readbacks_without_extra_clear_on_read_operations() {
    let mut io = Fake::new();
    let mut state = State::new();
    io.pg_extra = 5;
    io.enable_extra = 0x40;
    io.supply[7] = 0x80;
    io.supply[8] = 1;
    assert_eq!(state.measurement(), None);
    state.power_on(&mut io, 1600).unwrap();
    let active = state.measurement().unwrap();
    assert_eq!(active.attempt, 1);
    assert_eq!(active.started_ms, 0);
    assert_eq!(active.ready_ms, Some(io.now));
    assert_eq!(active.revision, Some(0x65));
    assert_eq!(active.temperature, Some(25));
    assert_eq!(active.enable_start, Some(0x60));
    assert_eq!(active.vcom1, Some(160));
    assert_eq!(active.vcom2, Some(4));
    assert_eq!(
        active.sequence,
        [Some(0xe4), Some(0x55), Some(0x1e), Some(0xe0)]
    );
    assert_eq!(active.faults_before, [Some(0x80), Some(1)]);
    assert_eq!(active.faults_active, [Some(0x80), Some(0)]);
    assert_eq!(active.power_good_start, Some(0xff));
    assert_eq!(active.power_good_active, Some(0xff));
    assert_eq!(active.shutdown_ms, None);
    io.shutdown(&mut state).unwrap();
    let finished = state.measurement().unwrap();
    assert_eq!(finished.power_good_off, Some(5));
    assert_eq!(finished.enable_off, Some(0x40));
    assert_eq!(finished.shutdown_ms, Some(io.now));
    for register in [7, 8] {
        assert_eq!(
            io.events
                .iter()
                .filter(|event| **event == Event::Read(SUPPLY, register))
                .count(),
            2
        );
    }
    for (register, count) in [(3, 1), (4, 2), (0x0f, 3)] {
        assert_eq!(
            io.events
                .iter()
                .filter(|event| **event == Event::Read(SUPPLY, register))
                .count(),
            count
        );
    }
    let events = io.events.len();
    assert_eq!(state.measurement(), Some(finished));
    assert_eq!(io.events.len(), events);
}

#[test]
fn a_failed_new_attempt_discards_previous_measurements() {
    let mut io = Fake::new();
    let mut state = State::new();
    state.power_on(&mut io, 1600).unwrap();
    io.shutdown(&mut state).unwrap();
    io.fail = Some(io.calls + 1);
    assert!(state.power_on(&mut io, 1600).is_err());
    let failed = state.measurement().unwrap();
    assert_eq!(failed.attempt, 2);
    assert_eq!(failed.started_ms, io.now);
    assert_eq!(failed.ready_ms, None);
    assert_eq!(failed.shutdown_ms, None);
    assert_eq!(failed.vcom1, None);
    assert_eq!(failed.power_good_start, None);
    assert_eq!(failed.enable_off, None);
}

#[test]
fn failed_activation_and_discharge_do_not_report_success() {
    let mut io = Fake::new();
    let mut state = State::new();
    io.activation_fault = 4;
    assert_eq!(state.power_on(&mut io, 1600), Err(Error::SupplyFault(0, 4)));
    let failed = state.measurement().unwrap();
    assert_eq!(failed.faults_active, [Some(0), Some(4)]);
    assert_eq!(failed.ready_ms, None);
    assert_eq!(failed.power_good_active, None);
    io.stuck_pg = true;
    assert!(io.shutdown(&mut state).is_err());
    let failed = state.measurement().unwrap();
    assert_eq!(failed.power_good_off, None);
    assert_eq!(failed.enable_off, None);
    assert_eq!(failed.shutdown_ms, None);
}

#[test]
fn control_inspection_reads_only_port_one_and_preserves_raw_values() {
    let mut io = Fake::new();
    io.expander[3] = 0x84;
    io.expander[5] = 0x40;
    let observed = inspect_controls(&mut io).unwrap();
    assert_eq!(observed.output, 0x84);
    assert_eq!(observed.input, 0xc4);
    assert_eq!(observed.configuration, 0xc4);
    assert_eq!(observed.inversion, 0x40);
    assert!(observed.controls_off());
    assert_eq!(
        io.events,
        [
            Event::Read(EXPANDER, 3),
            Event::Read(EXPANDER, 7),
            Event::Read(EXPANDER, 5),
            Event::Read(EXPANDER, 1),
        ]
    );
    io.expander[3] |= WAKE;
    assert!(!inspect_controls(&mut io).unwrap().controls_off());
    io.expander[3] &= !WAKE;
    io.expander[7] |= WAKE;
    assert!(!inspect_controls(&mut io).unwrap().controls_off());
}

#[test]
fn failed_control_inspection_never_attempts_recovery_writes() {
    for fail in 1..=4 {
        let mut io = Fake::new();
        io.fail = Some(fail);
        assert!(inspect_controls(&mut io).is_err());
        assert_eq!(io.events.len(), fail);
        assert!(
            io.events
                .iter()
                .all(|event| matches!(event, Event::Read(EXPANDER, _)))
        );
    }
}
