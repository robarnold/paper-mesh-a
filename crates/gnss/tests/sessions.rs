//! Verify serial attempts and reads retained across observation deadlines.
//!
//! See [serial sessions](../../../docs/platform/serial.md).

use paper_mesh_gnss::{Config, Receiver, ReceiverStorage, acquisition, probe};

use core::{
    future::{Future, poll_fn},
    num::{NonZeroU32, NonZeroU64},
    pin::Pin,
    task::{Context, Poll, Waker},
};
use paper_mesh_platform::location::{LocationAvailability, LocationSnapshot};
use paper_mesh_platform::serial::{ErrorType, Read, SerialReceiver};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}
fn complete<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    for _ in 0..1000 {
        if let Poll::Ready(value) = poll(future.as_mut()) {
            return value;
        }
    }
    panic!("future did not complete");
}
fn sentence(body: &str) -> Vec<u8> {
    let sum = body.bytes().fold(0, |sum, byte| sum ^ byte);
    format!("${body}*{sum:02X}\r\n").into_bytes()
}
const RATES: probe::BaudRates = probe::BaudRates {
    first: NonZeroU32::new(9600).unwrap(),
    remaining: &[NonZeroU32::new(38400).unwrap()],
};

struct Attempt {
    bytes: Vec<u8>,
    fail_open: bool,
    fail_read: bool,
}
#[derive(Default)]
struct Evidence {
    opened: Vec<u32>,
    closed: usize,
    pending_reads: usize,
}
struct Serial {
    attempts: VecDeque<Attempt>,
    evidence: Rc<RefCell<Evidence>>,
}
struct Session {
    bytes: VecDeque<u8>,
    fail_read: bool,
    evidence: Rc<RefCell<Evidence>>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.evidence.borrow_mut().closed += 1;
    }
}
impl ErrorType for Session {
    type Error = paper_mesh_platform::serial::ErrorKind;
}
impl Read for Session {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if self.fail_read {
            return Err(Self::Error::Other);
        }
        let Some(byte) = self.bytes.pop_front() else {
            self.evidence.borrow_mut().pending_reads += 1;
            return core::future::pending().await;
        };
        // Deliberately consume input before yielding: this read is not cancellation-safe.
        buf[0] = byte;
        embassy_futures::yield_now().await;
        Ok(1)
    }
}
impl SerialReceiver for Serial {
    type OpenError = ();
    type Session<'a> = Session;
    fn open(&mut self, baud: NonZeroU32) -> Result<Session, ()> {
        self.evidence.borrow_mut().opened.push(baud.get());
        let attempt = self.attempts.pop_front().unwrap();
        if attempt.fail_open {
            return Err(());
        }
        Ok(Session {
            bytes: attempt.bytes.into(),
            fail_read: attempt.fail_read,
            evidence: self.evidence.clone(),
        })
    }
}
fn serial(inputs: Vec<Vec<u8>>) -> Serial {
    Serial {
        attempts: inputs
            .into_iter()
            .map(|bytes| Attempt {
                bytes,
                fail_open: false,
                fail_read: false,
            })
            .collect(),
        evidence: Rc::new(RefCell::new(Evidence::default())),
    }
}

// Invalid input cannot select a rate; timeout closes one session before the next opens.
#[test]
fn probe_closes_failed_attempt_and_accepts_nmea_without_fix() {
    let mut serial = serial(vec![
        b"$GNGGA,bad*00\r\n".to_vec(),
        sentence("GNGGA,071557,,,,,0,00,,,,,,,"),
    ]);
    let evidence = serial.evidence.clone();
    let timeout_evidence = evidence.clone();
    let baud = complete(probe::find(&mut serial, RATES, move || {
        let evidence = timeout_evidence.clone();
        poll_fn(move |_| {
            if evidence.borrow().pending_reads > 0 && evidence.borrow().opened.len() == 1 {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
    }))
    .unwrap();
    assert_eq!(baud.get(), 38400);
    assert_eq!(evidence.borrow().opened, [9600, 38400]);
    assert_eq!(evidence.borrow().closed, 2);
}

// Cancellation closes a session even after input was consumed; retry starts a fresh envelope.
#[test]
fn canceled_probe_discards_partial_input() {
    let mut serial = serial(vec![
        sentence("GNTXT,01,01,02,old"),
        sentence("GNTXT,01,01,02,new"),
    ]);
    let evidence = serial.evidence.clone();
    {
        let mut probe = Box::pin(probe::find(&mut serial, RATES, core::future::pending::<()>));
        assert!(poll(probe.as_mut()).is_pending());
    }
    assert_eq!(evidence.borrow().closed, 1);
    assert_eq!(
        complete(probe::find(&mut serial, RATES, core::future::pending::<()>))
            .unwrap()
            .get(),
        9600
    );
    assert_eq!(evidence.borrow().closed, 2);
}

// Open failure leaves the factory available for retry and differs from no valid NMEA.
#[test]
fn failed_open_can_be_retried() {
    let mut serial = serial(vec![vec![], sentence("GNTXT,01,01,02,ready")]);
    serial.attempts[0].fail_open = true;
    assert!(matches!(
        complete(probe::find(&mut serial, RATES, core::future::pending::<()>)),
        Err(probe::Error::Open(()))
    ));
    assert_eq!(
        complete(probe::find(&mut serial, RATES, core::future::pending::<()>))
            .unwrap()
            .get(),
        9600
    );
}

// Wrong-baud errors reject candidates; exhausting the list returns no response.
#[test]
fn probe_reports_no_response_after_read_errors() {
    let mut serial = serial(vec![vec![], vec![]]);
    for attempt in &mut serial.attempts {
        attempt.fail_read = true;
    }
    assert!(matches!(
        complete(probe::find(&mut serial, RATES, core::future::pending::<()>)),
        Err(probe::Error::NoResponse)
    ));
    assert_eq!(serial.evidence.borrow().closed, 2);
}

struct SlowReader {
    first: Option<Vec<u8>>,
    second: Vec<u8>,
    ready: Rc<Cell<bool>>,
    calls: Rc<Cell<usize>>,
}
impl ErrorType for SlowReader {
    type Error = core::convert::Infallible;
}
impl Read for SlowReader {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        if let Some(bytes) = self.first.take() {
            buf[..bytes.len()].copy_from_slice(&bytes);
            return Ok(bytes.len());
        }
        buf[..self.second.len()].copy_from_slice(&self.second);
        poll_fn(|_| {
            if self.ready.get() {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;
        Ok(self.second.len())
    }
}

// Satellite expiry must preserve a read that has consumed bytes but has not completed.
#[test]
fn observation_deadline_preserves_in_flight_read() {
    let now = Cell::new(0);
    let ready = Rc::new(Cell::new(false));
    let calls = Rc::new(Cell::new(0));
    let mut reader = SlowReader {
        first: Some(sentence("GPGSV,1,1,01,01,45,180,42")),
        second: sentence("GNTXT,01,01,02,active"),
        ready: ready.clone(),
        calls: calls.clone(),
    };
    let mut storage = ReceiverStorage::EMPTY;
    let mut receiver = Receiver::new(
        &mut storage,
        Config {
            silence_ms: NonZeroU64::new(10000).unwrap(),
        },
    );
    let snapshots = RefCell::new(Vec::<LocationSnapshot>::new());
    let mut run = Box::pin(acquisition::run(
        &mut reader,
        &mut receiver,
        || now.get(),
        |deadline| {
            let now = &now;
            poll_fn(move |_| {
                if now.get() >= deadline {
                    Poll::Ready(())
                } else {
                    Poll::Pending
                }
            })
        },
        |snapshot| snapshots.borrow_mut().push(snapshot),
    ));
    assert!(poll(run.as_mut()).is_pending());
    assert!(poll(run.as_mut()).is_pending());
    assert_eq!(calls.get(), 2);
    now.set(5000);
    assert!(poll(run.as_mut()).is_pending());
    assert_eq!(calls.get(), 2);
    assert_eq!(snapshots.borrow().last().unwrap().satellites, None);
    ready.set(true);
    now.set(5001);
    assert!(poll(run.as_mut()).is_pending());
    assert_eq!(
        snapshots
            .borrow()
            .last()
            .unwrap()
            .activity
            .unwrap()
            .received_at_ms,
        5001
    );
    drop(run);
    receiver.reset();
    assert_eq!(receiver.snapshot, LocationSnapshot::new());
}

// Silence ends acquisition rather than changing baud or spinning on an expired timer.
#[test]
fn silence_ends_receive_session() {
    let mut serial = serial(vec![vec![]]);
    let mut session = serial.open(RATES.first).unwrap();
    let mut storage = ReceiverStorage::EMPTY;
    let mut receiver = Receiver::new(
        &mut storage,
        Config {
            silence_ms: NonZeroU64::new(10).unwrap(),
        },
    );
    let now = Cell::new(0);
    let end = complete(acquisition::run(
        &mut session,
        &mut receiver,
        || now.get(),
        |deadline| {
            now.set(deadline);
            core::future::ready(())
        },
        |_| {},
    ));
    assert!(matches!(end, acquisition::End::Silent));
    assert_eq!(
        receiver.snapshot.availability,
        LocationAvailability::Unavailable
    );
    drop(session);
    assert_eq!(serial.evidence.borrow().closed, 1);
}
