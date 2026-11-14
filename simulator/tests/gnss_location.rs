//! Verify portable GNSS observations against application age policy.
//!
//! See [location observations](../../docs/services/location.md).

use core::num::NonZeroU64;
use paper_mesh_gnss::{Config, Receiver, ReceiverStorage};
use paper_mesh_platform::location::{Location, LocationSnapshot};

const SILENCE_MS: u64 = 10_000;
const CONFIG: Config = Config {
    silence_ms: NonZeroU64::new(SILENCE_MS).unwrap(),
};

fn sentence(body: &str) -> Vec<u8> {
    let checksum = body.bytes().fold(0, |sum, byte| sum ^ byte);
    format!("${body}*{checksum:02X}\r\n").into_bytes()
}

fn feed(receiver: &mut Receiver<'_>, bytes: &[u8], time: u64) -> usize {
    bytes
        .iter()
        .filter(|byte| receiver.feed(**byte, time))
        .count()
}

const FIX: &str = "GNGGA,071556.000,3149.29103,N,11706.92916,E,1,21,0.7,75.7,M,-5.0,M,,";
const RMC: &str = "GNRMC,071556.000,A,3149.29103,N,11706.92916,E,10.00,315.25,250420,,,A,V";

struct Observed(LocationSnapshot);

impl Location for Observed {
    fn snapshot(&mut self, _now_ms: u64) -> LocationSnapshot {
        self.0
    }
}

// Position updates must not extend the application age limits for motion and UTC.
#[test]
fn gga_and_other_messages_cannot_keep_motion_or_utc_current() {
    use paper_mesh_app::location::{LocationScreen, ReceiverStatus};
    let mut storage = ReceiverStorage::EMPTY;
    let mut receiver = Receiver::new(&mut storage, CONFIG);
    let mut screen = LocationScreen::new();
    screen.open();
    feed(&mut receiver, &sentence(RMC), 100);
    screen.poll(100, &mut Observed(receiver.snapshot));
    assert_eq!(screen.view().speed_mm_s, Some(5_144));
    assert_eq!(screen.view().coordinates, None);
    assert_eq!(screen.next_deadline_ms(), Some(5_100));
    feed(&mut receiver, &sentence(FIX), 5_000);
    screen.poll(5_099, &mut Observed(receiver.snapshot));
    assert!(screen.view().utc.is_some());
    screen.poll(5_100, &mut Observed(receiver.snapshot));
    assert_eq!(screen.view().speed_mm_s, None);
    assert_eq!(screen.view().course_millidegrees, None);
    assert_eq!(screen.view().utc, None);
    assert!(screen.view().coordinates.is_some());
    assert_eq!(screen.view().receiver_status, ReceiverStatus::Active);
    feed(&mut receiver, &sentence(RMC), 5_101);
    screen.poll(5_101, &mut Observed(receiver.snapshot));
    assert!(screen.view().utc.is_some());
}
