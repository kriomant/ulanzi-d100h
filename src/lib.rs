//! Ulanzi D100H protocol, recovered from firmware 1.3.8.1 captures.
//!
//! The core codec is dependency-free. Enable `ble` for a btleplug client.
//! Characteristic UUIDs and a minimum handshake are not yet verified.
#![forbid(unsafe_code)]

pub mod protocol;
#[cfg(feature = "ble")]
pub mod ble;

pub use protocol::{Button, Command, Direction, Event, Message, Packet, ProtocolError, Reply};
