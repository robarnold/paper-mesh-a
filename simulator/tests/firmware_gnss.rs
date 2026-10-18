//! Native tests exercise the decoder and baud selection from the firmware.

#[path = "../../firmware/src/gnss/gsv.rs"]
mod gsv;
#[path = "../../firmware/src/gnss/power.rs"]
mod power;
#[path = "../../firmware/src/gnss/receiver.rs"]
mod receiver;

use embedded_hal::i2c::{ErrorKind, ErrorType, I2c, Operation};
use paper_mesh_platform::location::{
    Coordinates, Location, LocationAvailability, LocationSnapshot, SatelliteSystem, UtcTime,
};
use receiver::{PROBE_MS, Receiver, SILENCE_MS};

fn sentence(body: &str) -> Vec<u8> {
    let checksum = body.bytes().fold(0, |sum, byte| sum ^ byte);
    format!("${body}*{checksum:02X}\r\n").into_bytes()
}

fn feed(receiver: &mut Receiver, bytes: &[u8], time: u64) -> usize {
    bytes
        .iter()
        .filter(|byte| receiver.feed(**byte, time))
        .count()
}

const FIX: &str = "GNGGA,071556.000,3149.29103,N,11706.92916,E,1,21,0.7,75.7,M,-5.0,M,,";
const RMC: &str = "GNRMC,071556.000,A,3149.29103,N,11706.92916,E,10.00,315.25,250420,,,A,V";

#[test]
fn vendor_rmc_example_decodes_across_arbitrary_uart_chunks() {
    // Quectel L76K protocol specification V1.1, section 2.2.1, page 11.
    let bytes = b"$GNRMC,071556.000,A,3149.29103,N,11706.92916,E,0.00,0.00,250420,,,A,V*09\r\n";
    for split in 0..bytes.len() {
        let mut receiver = Receiver::INITIAL;
        feed(&mut receiver, &bytes[..split], 10);
        assert_eq!(feed(&mut receiver, &bytes[split..], 20), 1);
        let motion = receiver.snapshot.motion.unwrap();
        assert_eq!(motion.speed_mm_s, Some(0));
        assert_eq!(motion.course_millidegrees, Some(0));
        assert_eq!(motion.observed_at_ms, 20);
        assert_eq!(
            receiver.snapshot.utc.unwrap().time,
            UtcTime::new(7, 15, 56).unwrap()
        );
        assert_eq!(receiver.snapshot.observation, None);
    }
}

#[test]
fn rmc_converts_knots_and_accepts_optional_fields_and_talkers() {
    for talker in ["GP", "GN", "GL", "GA", "GB", "BD", "GQ", "GI"] {
        for suffix in [",,,A,V", ",,,D", ",,"] {
            let mut receiver = Receiver::INITIAL;
            let body = RMC.replace("GN", talker).replace(",,,A,V", suffix);
            feed(&mut receiver, &sentence(&body), 12);
            let motion = receiver.snapshot.motion.unwrap();
            assert_eq!(motion.speed_mm_s, Some(5_144));
            assert_eq!(motion.course_millidegrees, Some(315_250));
        }
    }
    for (speed, course, expected_speed, expected_course) in [
        ("0", "0", Some(0), Some(0)),
        ("1", "359.9999", Some(514), Some(359_999)),
        ("1.852", "90.5", Some(953), Some(90_500)),
        ("1.0009", "0.001", Some(514), Some(1)),
        ("", "", None, None),
        ("-1", "-1", None, None),
        ("NaN", "360", None, None),
        ("1e2", "360.001", None, None),
        ("12.", "1.2.3", None, None),
        ("+1", "+1", None, None),
        ("4294968", "4294968", None, None),
        ("999999999999", "999999999999", None, None),
    ] {
        let mut receiver = Receiver::INITIAL;
        let body = RMC.replace("10.00,315.25", &format!("{speed},{course}"));
        feed(&mut receiver, &sentence(&body), 1);
        let motion = receiver.snapshot.motion.unwrap();
        assert_eq!(motion.speed_mm_s, expected_speed, "{body}");
        assert_eq!(motion.course_millidegrees, expected_course, "{body}");
        assert!(receiver.snapshot.utc.is_some());
    }
}

#[test]
fn rmc_invalid_status_and_non_satellite_modes_clear_previous_values() {
    for (status, mode) in [
        ("V", "A"),
        ("A", "N"),
        ("A", "E"),
        ("A", "S"),
        ("A", "M"),
        ("A", ""),
    ] {
        let mut receiver = Receiver::INITIAL;
        feed(&mut receiver, &sentence(RMC), 1);
        let body = RMC
            .replace(",A,", &format!(",{status},"))
            .replace(",,,A,V", &format!(",,,{mode},V"));
        feed(&mut receiver, &sentence(&body), 2);
        let motion = receiver.snapshot.motion.unwrap();
        assert_eq!(motion.speed_mm_s, None, "{body}");
        assert_eq!(motion.course_millidegrees, None, "{body}");
        assert_eq!(receiver.snapshot.utc, None, "{body}");
        assert_eq!(motion.observed_at_ms, 2);
    }
    for (status, mode) in [("D", "D"), ("A", "F"), ("A", "R")] {
        let mut receiver = Receiver::INITIAL;
        let body = RMC
            .replace(",A,", &format!(",{status},"))
            .replace(",,,A,V", &format!(",,,{mode},V"));
        feed(&mut receiver, &sentence(&body), 2);
        assert!(receiver.snapshot.motion.unwrap().speed_mm_s.is_some());
        assert!(receiver.snapshot.utc.is_some());
    }
}

#[test]
fn utc_rejects_invalid_time_without_discarding_motion_and_accepts_midnight() {
    let mut receiver = Receiver::INITIAL;
    for (time, expected) in [
        ("235959.999", UtcTime::new(23, 59, 59)),
        ("000000", UtcTime::new(0, 0, 0)),
        ("", None),
        ("240000", None),
        ("126000", None),
        ("125960", None),
        ("12345", None),
        ("1234567", None),
        ("123456.", None),
        ("123456.NaN", None),
        ("-12345", None),
    ] {
        feed(&mut receiver, &sentence(RMC), 1);
        let body = RMC.replace("071556.000", time);
        feed(&mut receiver, &sentence(&body), 2);
        assert_eq!(
            receiver.snapshot.utc.map(|value| value.time),
            expected,
            "{time}"
        );
        assert_eq!(receiver.snapshot.motion.unwrap().speed_mm_s, Some(5_144));
    }
}

#[test]
fn malformed_rmc_cannot_refresh_observations_and_transport_faults_clear_them() {
    let mut receiver = Receiver::INITIAL;
    feed(&mut receiver, &sentence(RMC), 10);
    let previous = receiver.snapshot;
    for body in [
        RMC.replace(",A,", ",X,"),
        format!("{RMC},extra"),
        "GNRMC,071556,A".into(),
    ] {
        feed(&mut receiver, &sentence(&body), 20);
        assert_eq!(receiver.snapshot.motion, previous.motion);
        assert_eq!(receiver.snapshot.utc, previous.utc);
    }
    assert_eq!(
        feed(&mut receiver, format!("${RMC}*00\r\n").as_bytes(), 20),
        0
    );
    assert_eq!(receiver.snapshot.motion, previous.motion);
    assert_eq!(receiver.snapshot.utc, previous.utc);
    receiver.uart_error();
    assert_eq!(receiver.snapshot.motion, None);
    assert_eq!(receiver.snapshot.utc, None);
    feed(&mut receiver, &sentence(RMC), 30);
    receiver.advance(30 + SILENCE_MS);
    assert_eq!(receiver.snapshot.motion, None);
    assert_eq!(receiver.snapshot.utc, None);
}

impl Location for Receiver {
    fn snapshot(&mut self, _now_ms: u64) -> LocationSnapshot {
        self.snapshot
    }
}

#[test]
fn gga_and_other_messages_cannot_keep_motion_or_utc_current() {
    use paper_mesh_app::location::{LocationScreen, ReceiverStatus};
    let mut receiver = Receiver::INITIAL;
    let mut screen = LocationScreen::new();
    screen.open();
    feed(&mut receiver, &sentence(RMC), 100);
    screen.poll(100, &mut receiver);
    assert_eq!(screen.view().speed_mm_s, Some(5_144));
    assert_eq!(screen.view().coordinates, None);
    assert_eq!(screen.next_deadline_ms(), Some(5_100));
    feed(&mut receiver, &sentence(FIX), 5_000);
    screen.poll(5_099, &mut receiver);
    assert!(screen.view().utc.is_some());
    screen.poll(5_100, &mut receiver);
    assert_eq!(screen.view().speed_mm_s, None);
    assert_eq!(screen.view().course_millidegrees, None);
    assert_eq!(screen.view().utc, None);
    assert!(screen.view().coordinates.is_some());
    assert_eq!(screen.view().receiver_status, ReceiverStatus::Active);
    feed(&mut receiver, &sentence(RMC), 5_101);
    screen.poll(5_101, &mut receiver);
    assert!(screen.view().utc.is_some());
}

#[test]
fn vendor_gga_example_decodes_across_arbitrary_uart_chunks() {
    // Quectel L76K protocol specification V1.1, section 2.2.2, page 13.
    let bytes = b"$GNGGA,071556.000,3149.29103,N,11706.92916,E,1,21,0.7,75.7,M,-5.0,M,,*69\r\n";
    for split in 0..bytes.len() {
        let mut receiver = Receiver::INITIAL;
        assert_eq!(feed(&mut receiver, &bytes[..split], 10), 0);
        assert_eq!(feed(&mut receiver, &bytes[split..], 20), 1);
        let observation = receiver.snapshot.observation.unwrap();
        assert_eq!(
            observation.coordinates,
            Coordinates::new(31_821_517, 117_115_486)
        );
        assert_eq!(observation.satellites_used, Some(21));
        assert_eq!(observation.elevation_mm, Some(75_700));
        assert_eq!(observation.observed_at_ms, 20);
        assert_eq!(receiver.snapshot.availability, LocationAvailability::Ready);
    }
}

#[test]
fn coordinates_keep_hemispheres_zero_and_boundary_values() {
    for (latitude, ns, longitude, ew, expected) in [
        ("0030.00006", "S", "00030.00006", "W", (-500_001, -500_001)),
        ("0000.00000", "N", "00000.00000", "E", (0, 0)),
        (
            "9000.00000",
            "S",
            "18000.00000",
            "W",
            (-90_000_000, -180_000_000),
        ),
    ] {
        let mut receiver = Receiver::INITIAL;
        let body = format!("GPGGA,000000,{latitude},{ns},{longitude},{ew},1,08,1.0,0,M,0,M,,");
        assert_eq!(feed(&mut receiver, &sentence(&body), 5), 1);
        assert_eq!(
            receiver.snapshot.observation.unwrap().coordinates,
            Coordinates::new(expected.0, expected.1)
        );
    }
}

#[test]
fn absent_and_non_satellite_fixes_clear_previous_coordinates() {
    for quality in [0, 3, 6, 7, 8] {
        let mut receiver = Receiver::INITIAL;
        feed(&mut receiver, &sentence(FIX), 1);
        let body = format!("GNGGA,071557,,,,,{quality},00,,,,,,,");
        assert_eq!(feed(&mut receiver, &sentence(&body), 2), 1);
        let observation = receiver.snapshot.observation.unwrap();
        assert_eq!(observation.coordinates, None);
        assert_eq!(observation.elevation_mm, None);
        assert_eq!(observation.satellites_used, Some(0));
        assert_eq!(observation.observed_at_ms, 2);
    }
}

#[test]
fn elevation_accepts_signed_meters_and_clears_invalid_or_missing_values() {
    let mut receiver = Receiver::INITIAL;
    for (text, unit, expected) in [
        ("75.7", "M", Some(75_700)),
        ("-430.5", "M", Some(-430_500)),
        ("-0.125", "M", Some(-125)),
        ("0", "M", Some(0)),
        ("+12.3456", "M", Some(12_345)),
        ("2147483.647", "M", Some(i32::MAX)),
        ("-2147483.648", "M", Some(i32::MIN)),
        ("", "M", None),
        ("75.7", "", None),
        ("75.7", "F", None),
        ("nan", "M", None),
        ("1e3", "M", None),
        ("12.", "M", None),
        ("1.2.3", "M", None),
        ("2147483.648", "M", None),
        ("-2147483.649", "M", None),
        ("999999999999999999999", "M", None),
    ] {
        feed(&mut receiver, &sentence(FIX), 1);
        let body = FIX.replace("75.7,M", &format!("{text},{unit}"));
        feed(&mut receiver, &sentence(&body), 2);
        let observation = receiver.snapshot.observation.unwrap();
        assert_eq!(observation.elevation_mm, expected, "{text},{unit}");
        assert!(observation.coordinates.is_some());
        assert_eq!(observation.observed_at_ms, 2);
    }
    // A reported height cannot make an invalid position usable.
    for quality in [0, 3, 6, 7, 8] {
        let body = FIX.replace(",1,21,", &format!(",{quality},21,"));
        feed(&mut receiver, &sentence(&body), 3);
        assert_eq!(receiver.snapshot.observation.unwrap().elevation_mm, None);
    }
}

#[test]
fn malformed_reports_cannot_refresh_a_previous_fix() {
    let mut receiver = Receiver::INITIAL;
    feed(&mut receiver, &sentence(FIX), 10);
    let previous = receiver.snapshot;
    for body in [
        FIX.replace("3149.29103", "3160.00000"),
        FIX.replace("3149.29103", "9000.00001"),
        FIX.replace("11706.92916", "18000.00001"),
        FIX.replace(",N,", ",E,"),
        FIX.replace("3149.29103", "31é9.29103"),
        FIX.replace("3149.29103", ""),
        FIX.replace("3149.29103", "3149.nan"),
        FIX.replace(",1,21,", ",9,21,"),
        FIX.replace(",1,21,", ",1,100,"),
        FIX.replace("GNGGA", "GNRMC"),
        format!("{FIX},extra"),
        "GNGGA,071556,3149.29103,N,11706.92916,E,1,21".into(),
    ] {
        assert_eq!(
            feed(&mut receiver, &sentence(&body), 20),
            usize::from(body.is_ascii()),
            "{body}"
        );
        assert_eq!(receiver.snapshot.observation, previous.observation);
    }
    let previous = receiver.snapshot;
    assert_eq!(
        feed(&mut receiver, format!("${FIX}*00\r\n").as_bytes(), 20),
        0
    );
    assert_eq!(feed(&mut receiver, format!("${FIX}\r\n").as_bytes(), 20), 0);
    assert_eq!(receiver.snapshot, previous);
}

#[test]
fn vendor_gsv_groups_publish_only_after_completion_and_without_a_fix() {
    // Quectel L76K protocol specification V1.1, section 2.2.3, pages 14-15.
    let lines: [&[u8]; 3] = [
        b"$GPGSV,3,1,12,02,49,123,43,05,60,005,43,06,06,127,29,07,05,042,33,0*66\r\n",
        b"$GPGSV,3,2,12,13,79,139,44,15,55,225,44,18,12,314,36,20,44,055,42,0*6C\r\n",
        b"$GPGSV,3,3,12,29,48,275,42,30,20,069,38,193,69,062,42,195,46,160,42,0*6A\r\n",
    ];
    for split in 0..lines[0].len() {
        let mut receiver = Receiver::INITIAL;
        feed(&mut receiver, &lines[0][..split], 100);
        feed(&mut receiver, &lines[0][split..], 110);
        assert_eq!(receiver.snapshot.satellites, None);
        feed(&mut receiver, lines[1], 120);
        assert_eq!(receiver.snapshot.satellites, None);
        feed(&mut receiver, lines[2], 130);
        let satellites = receiver.snapshot.satellites.unwrap();
        assert_eq!(satellites.satellites_in_view, 12);
        assert_eq!(satellites.satellites_with_signal, 12);
        assert_eq!(satellites.observed_at_ms, 130);
        assert_eq!(satellites.strongest[0].unwrap().id, 13);
        assert_eq!(satellites.strongest[0].unwrap().cn0_db_hz, Some(44));
        assert_eq!(receiver.snapshot.observation, None);
        assert_eq!(receiver.snapshot.activity.unwrap().valid_messages, 3);
        assert_eq!(receiver.snapshot.availability, LocationAvailability::Ready);
    }
}

#[test]
fn satellite_counts_deduplicate_signals_and_keep_constellations_separate() {
    let mut receiver = Receiver::INITIAL;
    for (body, time) in [
        (
            "GPGSV,2,1,05,01,45,180,10,02,30,090,,03,20,100,00,04,10,050,35,1",
            1,
        ),
        ("BDGSV,1,1,01,01,45,180,40,1", 2),
        ("GPGSV,1,1,01,01,45,180,42,2", 3),
        ("GPGSV,2,2,05,05,10,050,20,1", 4),
        ("GBGSV,1,1,01,01,45,180,39,1", 5),
    ] {
        feed(&mut receiver, &sentence(body), time);
    }
    let satellites = receiver.snapshot.satellites.unwrap();
    assert_eq!(satellites.satellites_in_view, 6);
    assert_eq!(satellites.satellites_with_signal, 4);
    assert_eq!(satellites.strongest[0].unwrap().cn0_db_hz, Some(42));
    assert_eq!(
        satellites.strongest[1].unwrap().system,
        SatelliteSystem::BeiDou
    );
    assert_eq!(satellites.strongest[4].unwrap().cn0_db_hz, Some(0));
    assert_eq!(satellites.strongest[5].unwrap().cn0_db_hz, None);
}

#[test]
fn incomplete_malformed_duplicate_and_old_gsv_groups_cannot_publish() {
    let first = "GPGSV,2,1,05,01,45,180,10,02,30,090,,03,20,100,00,04,10,050,35,1";
    let last = "GPGSV,2,2,05,05,10,050,20,1";
    for bad in [
        "GPGSV,2,2,05,01,10,050,20,1", // Duplicate satellite.
        "GPGSV,2,2,05,05,91,050,20,1",
        "GPGSV,2,2,05,05,10,360,20,1",
        "GPGSV,2,2,05,05,10,050,100,1",
        "GPGSV,2,2,06,05,10,050,20,06,10,050,21,1",
        "GPGSV,3,3,09,09,10,050,20,1",
    ] {
        let mut receiver = Receiver::INITIAL;
        feed(&mut receiver, &sentence(first), 1);
        feed(&mut receiver, &sentence(bad), 2);
        feed(&mut receiver, &sentence(last), 3);
        assert_eq!(receiver.snapshot.satellites, None, "{bad}");
    }
    let mut receiver = Receiver::INITIAL;
    feed(&mut receiver, &sentence(last), 1);
    assert_eq!(receiver.snapshot.satellites, None);
    feed(&mut receiver, &sentence(first), 2);
    feed(&mut receiver, &sentence(last), 5_002);
    assert_eq!(receiver.snapshot.satellites, None);
    feed(&mut receiver, &sentence(first), 5_003);
    feed(&mut receiver, &sentence(last), 5_004);
    assert_eq!(receiver.snapshot.satellites.unwrap().satellites_in_view, 5);
}

#[test]
fn satellite_reports_expire_while_other_messages_keep_receiver_active() {
    let mut receiver = Receiver::INITIAL;
    feed(&mut receiver, &sentence("GPGSV,1,1,01,01,45,180,42"), 10);
    feed(&mut receiver, &sentence(FIX), 20);
    feed(
        &mut receiver,
        &sentence("GNTXT,01,01,02,Receiver active"),
        4_000,
    );
    assert_eq!(receiver.deadline_ms(), 5_010);
    assert!(receiver.advance(5_010));
    assert_eq!(receiver.baud(), 9_600);
    assert_eq!(receiver.snapshot.satellites, None);
    assert_eq!(receiver.snapshot.observation.unwrap().observed_at_ms, 20);
    assert_eq!(receiver.snapshot.activity.unwrap().received_at_ms, 4_000);
    assert_eq!(receiver.snapshot.availability, LocationAvailability::Ready);
    let previous = receiver.snapshot.activity;
    feed(&mut receiver, b"$GNTXT,01,01,02,noise*00\r\n", 5_020);
    feed(&mut receiver, b"noise\r\n", 5_021);
    assert_eq!(receiver.snapshot.activity, previous);
    feed(&mut receiver, &sentence("GPGSV,1,1,00,0"), 5_030);
    assert_eq!(receiver.snapshot.satellites.unwrap().satellites_in_view, 0);
    receiver.uart_error();
    assert_eq!(receiver.snapshot.satellites, None);
}

#[test]
fn gsv_storage_overflow_is_explicit_and_expires() {
    let mut receiver = Receiver::INITIAL;
    for signal in 0..13 {
        feed(
            &mut receiver,
            &sentence(&format!("GPGSV,1,1,01,01,45,180,42,{signal:X}")),
            1,
        );
    }
    let satellites = receiver.snapshot.satellites.unwrap();
    assert_eq!(satellites.satellites_in_view, 1);
    assert!(satellites.truncated);
    receiver.advance(5_001);
    assert_eq!(receiver.snapshot.satellites, None);
    feed(&mut receiver, &sentence("GPGSV,1,1,00"), 5_002);
    assert!(!receiver.snapshot.satellites.unwrap().truncated);
}

#[test]
fn noise_truncation_and_overflow_recover_at_the_next_sentence() {
    let mut receiver = Receiver::INITIAL;
    feed(&mut receiver, b"noise\xff$GNGGA,partial", 1);
    assert_eq!(feed(&mut receiver, &sentence(FIX), 2), 1);
    receiver.feed(b'$', 3);
    for _ in 0..2048 {
        receiver.feed(b'x', 3);
    }
    assert!(!receiver.feed(b'\n', 3));
    assert_eq!(feed(&mut receiver, &sentence(FIX), 4), 1);
    feed(&mut receiver, b"$GNGGA,partial", 5);
    receiver.uart_error();
    assert_eq!(receiver.snapshot.observation, None);
    assert_eq!(receiver.snapshot.availability, LocationAvailability::Fault);
    assert_eq!(feed(&mut receiver, &sentence(FIX), 6), 1);
    assert_eq!(receiver.snapshot.availability, LocationAvailability::Ready);
}

#[test]
fn baud_probes_retry_without_a_fix_and_recover_after_silence() {
    let mut receiver = Receiver::INITIAL;
    receiver.start_at(0);
    assert_eq!(receiver.baud(), 9_600);
    receiver.uart_error();
    assert_eq!(
        receiver.snapshot.availability,
        LocationAvailability::Starting
    );
    assert!(!receiver.advance(PROBE_MS - 1));
    assert!(receiver.advance(PROBE_MS));
    assert_eq!(receiver.baud(), 38_400);
    assert_eq!(
        receiver.snapshot.availability,
        LocationAvailability::Starting
    );
    assert!(receiver.advance(2 * PROBE_MS));
    assert_eq!(
        receiver.snapshot.availability,
        LocationAvailability::Unavailable
    );
    assert_eq!(receiver.baud(), 9_600);
    assert_eq!(
        feed(
            &mut receiver,
            &sentence("GNGGA,071557,,,,,0,00,,,,,,,"),
            8_100
        ),
        1
    );
    assert_eq!(receiver.snapshot.availability, LocationAvailability::Ready);
    assert_eq!(receiver.snapshot.observation.unwrap().coordinates, None);
    assert_eq!(receiver.deadline_ms(), 8_100 + SILENCE_MS);
    assert!(!receiver.advance(8_100 + SILENCE_MS - 1));
    assert!(receiver.advance(8_100 + SILENCE_MS));
    assert_eq!(receiver.snapshot.observation, None);
    assert_eq!(receiver.baud(), 38_400);
    assert_eq!(feed(&mut receiver, &sentence(FIX), 18_200), 1);
    assert_eq!(receiver.snapshot.availability, LocationAvailability::Ready);
}

struct Expander {
    registers: [u8; 8],
    transactions: usize,
    fail_at: Option<usize>,
}

impl ErrorType for Expander {
    type Error = ErrorKind;
}

impl I2c for Expander {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [Operation<'_>],
    ) -> Result<(), Self::Error> {
        assert_eq!(address, 0x20);
        self.transactions += 1;
        if self.fail_at == Some(self.transactions) {
            return Err(ErrorKind::Other);
        }
        match operations {
            [Operation::Write(register), Operation::Read(value)] => {
                value[0] = self.registers[register[0] as usize];
            }
            [Operation::Write(bytes)] => {
                self.registers[bytes[0] as usize] = bytes[1];
                // An enabled output must never drive the shared supply low.
                if self.registers[6] & 1 == 0 {
                    assert_eq!(self.registers[2] & 1, 1);
                }
            }
            _ => panic!("unexpected I2C transaction"),
        }
        Ok(())
    }
}

#[test]
fn power_enable_preserves_display_controls_and_recovers_after_each_bus_failure() {
    let original = [0x12, 0x34, 0xaa, 0x5b, 0xcd, 0xef, 0xff, 0xc4];
    for fail_at in [None, Some(1), Some(2), Some(3), Some(4)] {
        let mut bus = Expander {
            registers: original,
            transactions: 0,
            fail_at,
        };
        assert_eq!(power::enable(&mut bus).is_err(), fail_at.is_some());
        bus.fail_at = None;
        power::enable(&mut bus).unwrap();
        power::enable(&mut bus).unwrap();
        let mut expected = original;
        expected[2] |= 1;
        expected[6] &= !1;
        assert_eq!(bus.registers, expected);
    }
}
