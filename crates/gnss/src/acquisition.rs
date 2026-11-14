//! Receive-session decoding with observation deadlines.
//!
//! # Design choices
//!
//! - Retain pending reads across expiry checks so ordinary timers cannot discard bytes.
//! - End the session on silence, EOF, or I/O failure; firmware chooses recovery.
//!
//! See [GNSS input](../../../docs/adapters/gnss.md).

use super::receiver::Receiver;
use core::{future::Future, pin::pin};
use embassy_futures::{
    select::{Either, select},
    yield_now,
};
use paper_mesh_platform::{
    location::{LocationAvailability, LocationSnapshot},
    serial::Read,
};

/// Reason the caller must close this receive session.
#[derive(Debug)]
pub enum End<E> {
    Read(E),
    Closed,
    Silent,
}

/// Decode one session. A canceled call requires session closure and decoder reset.
pub async fn run<R: Read, F: Future<Output = ()>>(
    reader: &mut R,
    receiver: &mut Receiver<'_>,
    now: impl Fn() -> u64,
    mut deadline: impl FnMut(u64) -> F,
    mut publish: impl FnMut(LocationSnapshot),
) -> End<R::Error> {
    receiver.reset();
    receiver.start_at(now());
    publish(receiver.snapshot);
    let mut bytes = [0; 128];
    loop {
        let result = {
            let mut read = pin!(reader.read(&mut bytes));
            loop {
                match select(deadline(receiver.deadline_ms()), read.as_mut()).await {
                    Either::Second(result) => break result,
                    Either::First(()) => {
                        if receiver.advance(now()) {
                            publish(receiver.snapshot);
                        }
                        if receiver.snapshot.availability == LocationAvailability::Unavailable {
                            return End::Silent;
                        }
                        // Keep this same read future alive across observation expiry.
                    }
                }
            }
        };
        let length = match result {
            Ok(0) => {
                receiver.uart_error();
                publish(receiver.snapshot);
                return End::Closed;
            }
            Ok(length) => length,
            Err(error) => {
                receiver.uart_error();
                publish(receiver.snapshot);
                return End::Read(error);
            }
        };
        let at_ms = now();
        let mut changed = false;
        for byte in &bytes[..length] {
            changed |= receiver.feed(*byte, at_ms);
        }
        changed |= receiver.advance(at_ms);
        if changed {
            publish(receiver.snapshot);
        }
        if receiver.snapshot.availability == LocationAvailability::Unavailable {
            return End::Silent;
        }
        yield_now().await;
    }
}
