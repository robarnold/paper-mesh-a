//! BQ25896 observations and separate battery-isolation operations.
//!
//! # Design choices
//!
//! - Keep observation access separate from commands that can interrupt system power.
//! - Leave bus ownership, service coordination, and reset recovery with the runtime.
//!
//! See [BQ25896 adapter](../../../docs/adapters/bq25896.md).

#![no_std]

pub mod observation;
pub mod ship;

// The BQ25896 uses the chip-defined seven-bit address 0x6b. Board wiring does not select it.
// The bus supplies the direction bit. See datasheet section 6 in the adapter documentation.
const I2C_ADDRESS: u8 = 0x6b;
