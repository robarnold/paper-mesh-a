//! Bounded startup detection of a working NMEA baud rate.
//!
//! # Design choices
//!
//! - Open a fresh session per candidate and accept a valid envelope without requiring a satellite fix.
//! - Let firmware supply attempt deadlines and decide when to retry a failed probe.
//!
//! See [GNSS acquisition](../../../docs/adapters/gnss.md#acquisition).

use super::nmea::Sentence;
use core::{future::Future, num::NonZeroU32};
use embassy_futures::{
    select::{Either, select},
    yield_now,
};
use paper_mesh_platform::serial::{Read, SerialReceiver};

/// Ordered, nonempty candidates for one bounded probe.
#[derive(Clone, Copy)]
pub struct BaudRates {
    pub first: NonZeroU32,
    pub remaining: &'static [NonZeroU32],
}

/// Failure to configure a candidate, or absence of valid NMEA at all candidates.
#[derive(Debug)]
pub enum Error<E> {
    Open(E),
    NoResponse,
}

/// Return a working host baud without changing the receiver's configuration.
/// Each deadline starts after opening its candidate. Cancellation drops the active session.
/// Read errors and end-of-stream reject a candidate. No response does not identify the receiver model.
pub async fn find<S: SerialReceiver, F: Future<Output = ()>>(
    serial: &mut S,
    rates: BaudRates,
    mut deadline: impl FnMut() -> F,
) -> Result<NonZeroU32, Error<S::OpenError>> {
    for baud in core::iter::once(rates.first).chain(rates.remaining.iter().copied()) {
        let mut session = serial.open(baud).map_err(Error::Open)?;
        if matches!(
            select(deadline(), valid_sentence(&mut session)).await,
            Either::Second(Ok(()))
        ) {
            return Ok(baud);
        }
    }
    Err(Error::NoResponse)
}

async fn valid_sentence(reader: &mut impl Read) -> Result<(), ()> {
    let mut sentence = Sentence::default();
    let mut bytes = [0; 128];
    loop {
        let length = reader.read(&mut bytes).await.map_err(|_| ())?;
        if length == 0 {
            return Err(());
        }
        if bytes[..length]
            .iter()
            .any(|byte| sentence.feed(*byte).is_some())
        {
            return Ok(());
        }
        // Continuous noise must not starve the deadline or suspend request.
        yield_now().await;
    }
}
