#![no_std]

//! Portable NMEA reception and location observations.
//!
//! # Design choices
//!
//! - Borrow serial sessions and caller-owned storage without selecting hardware or an executor.
//! - Keep optional baud probing separate from normal acquisition and observation expiry.
//!
//! See [GNSS receiver](../../../docs/adapters/gnss.md).

mod config;
mod gsv;
mod nmea;
mod receiver;

pub mod acquisition;
pub mod probe;

pub use config::Config;
pub use receiver::{Receiver, ReceiverStorage};
