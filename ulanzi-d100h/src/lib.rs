//! Ulanzi D100H protocol, recovered from firmware 1.3.8.1 captures.
//!
//! BLE communication via btleplug, with a separate dependency-free protocol crate.
//! Characteristic UUIDs and a minimum handshake are not yet verified.
#![forbid(unsafe_code)]

pub use ulanzi_d100h_protocol as protocol;
pub mod ble;

pub use protocol::{Button, Command, Direction, Event, Message, Packet, ProtocolError, Reply};
