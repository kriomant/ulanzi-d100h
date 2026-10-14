//! Ulanzi D100H protocol, recovered from firmware 1.3.8.1 captures.
//!
//! BLE communication via btleplug, with a separate dependency-free protocol crate.
//! Confirmed vendor channel: service 1971, write fd01, notify fd00.
//! Subscription plus immediate and periodic heartbeats enables online events,
//! including after a power cycle on the tested already paired firmware 1.3.8.1.
#![forbid(unsafe_code)]

pub use ulanzi_d100h_protocol as protocol;
pub mod ble;

pub use protocol::{BatteryStatus, Button, Command, Direction, Event, FirmwareVersion, Message, Packet, ProtocolError, Reply};
