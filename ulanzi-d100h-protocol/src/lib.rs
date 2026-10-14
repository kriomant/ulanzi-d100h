//! Ulanzi D100H packet codec, recovered from firmware 1.3.8.1 captures.
#![forbid(unsafe_code)]

use std::{error::Error, fmt, time::Duration};

pub const PACKET_LEN: usize = 20;
const KEY: [u32; 4] = [0xcaa5baca, 0xbc2a8a6d, 0xca5a9eba, 0x9bb88bca];
const DELTA: u32 = 0x9e3779b9;

/// A plaintext vendor message. Encryption covers its first two 8-byte blocks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Packet(pub [u8; PACKET_LEN]);

impl Packet {
    pub fn from_wire(wire: &[u8]) -> Result<Self, ProtocolError> {
        let mut bytes: [u8; PACKET_LEN] = wire.try_into()
            .map_err(|_| ProtocolError::InvalidLength(wire.len()))?;
        crypt(&mut bytes, false);
        Ok(Self(bytes))
    }

    pub fn to_wire(self) -> [u8; PACKET_LEN] {
        let mut bytes = self.0;
        crypt(&mut bytes, true);
        bytes
    }

    /// Unknown messages are retained verbatim rather than assigned speculative meanings.
    pub fn message(self) -> Message {
        let p = self.0;
        if p[..2] == [0x8b, 0x10] && p[4..].iter().all(|&b| b == 0) {
            if let Ok(button) = Button::try_from(p[2]) && p[3] <= 1 {
                return Message::Event(Event::Button { button, pressed: p[3] == 1 });
            }
            if p[2] == 7 && p[3] <= 1 {
                return Message::Event(Event::KnobClick { pressed: p[3] == 1 });
            }
            if (9..=12).contains(&p[2]) && p[3] == 1 {
                return Message::Event(Event::Rotation {
                    direction: if p[2] % 2 == 1 { Direction::Counterclockwise } else { Direction::Clockwise },
                    knob_pressed: p[2] >= 11,
                });
            }
        }
        if p[..2] == [0x0b, 0x7b] && p[4] & 0xe0 == 0
            && p[5] <= 100 && p[6..].iter().all(|&b| b == 0)
        {
            return Message::BatteryStatus(BatteryStatus {
                voltage_raw: u16::from_le_bytes([p[2], p[3]]),
                battery_percent: p[5],
                low_battery_status: p[4] & 0x03,
                power_off: p[4] & 0x04 != 0,
                charging: p[4] & 0x08 != 0,
                charge_full: p[4] & 0x10 != 0,
            });
        }
        match p[..4] {
            [6, 1, 0x23, 0x10] if matches!(p[4], 0 | 1) && p[5..].iter().all(|&b| b == 0) =>
                Message::Reply(Reply::Heartbeat),
            [1, 6, 0x40, op @ (0x11 | 0x14)] if matches!(p[4], 0 | 0xff)
                && p[5..].iter().all(|&b| b == 0) =>
                Message::Reply(Reply::Haptics { enabled: p[4] == 0xff, write_response: op == 0x14 }),
            [1, 1, 0x42, op @ (0x11 | 0x12)] if p[6..].iter().all(|&b| b == 0) =>
                Message::Reply(Reply::SleepTimer {
                    seconds: u16::from_le_bytes([p[4], p[5]]), write_response: op == 0x12,
                }),
            [1, 4, 4, 0x11] => Message::Reply(Reply::FirmwareVersion {
                version: FirmwareVersion { major: p[10], minor: p[11], patch: p[12],
                    revision: if p[18] == 0 { None } else { Some(p[18]) } },
                custom_code: [p[4], p[5], p[6]],
                raw_data: p[4..].try_into().unwrap(),
            }),
            [1, 1, 0xfa, 0x11] if p[10..].iter().all(|&b| b == 0) =>
                Message::Reply(Reply::DeviceAddress(p[4..10].try_into().unwrap())),
            [1, 4, 0x0b, 0x11] =>
                Message::Reply(Reply::FlashId(p[4..].try_into().unwrap())),
            [1, 1, 0x0b, 0x11] if p[4] <= 14 => {
                let length = usize::from(p[4]);
                Message::Reply(Reply::SerialFragment { index: p[5], text: p[6..6 + length].to_vec() })
            }
            _ => Message::Unknown(self),
        }
    }
}

fn crypt(bytes: &mut [u8; PACKET_LEN], encrypt: bool) {
    for block in bytes[..16].as_chunks_mut::<8>().0 {
        let mut a = u32::from_le_bytes(block[..4].try_into().unwrap());
        let mut b = u32::from_le_bytes(block[4..].try_into().unwrap());
        let mix = |v: u32, sum: u32, k0: u32, k1: u32| {
            (v << 4).wrapping_add(k0) ^ v.wrapping_add(sum) ^ (v >> 5).wrapping_add(k1)
        };
        let mut sum: u32 = if encrypt { 0 } else { 0xc6ef3720 };
        for _ in 0..32 {
            if encrypt {
                sum = sum.wrapping_add(DELTA);
                a = a.wrapping_add(mix(b, sum, KEY[0], KEY[1]));
                b = b.wrapping_add(mix(a, sum, KEY[2], KEY[3]));
            } else {
                b = b.wrapping_sub(mix(a, sum, KEY[2], KEY[3]));
                a = a.wrapping_sub(mix(b, sum, KEY[0], KEY[1]));
                sum = sum.wrapping_sub(DELTA);
            }
        }
        block[..4].copy_from_slice(&a.to_le_bytes());
        block[4..].copy_from_slice(&b.to_le_bytes());
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Button {
    TopLeft = 1, TopMiddle = 2, TopRight = 3,
    UpperLeft = 4, UpperRight = 5, LowerLeft = 6, LowerRight = 8,
}

impl TryFrom<u8> for Button {
    type Error = ProtocolError;
    fn try_from(id: u8) -> Result<Self, Self::Error> {
        match id {
            1 => Ok(Self::TopLeft), 2 => Ok(Self::TopMiddle), 3 => Ok(Self::TopRight),
            4 => Ok(Self::UpperLeft), 5 => Ok(Self::UpperRight), 6 => Ok(Self::LowerLeft),
            8 => Ok(Self::LowerRight), _ => Err(ProtocolError::UnknownButton(id)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction { Counterclockwise, Clockwise }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    Button { button: Button, pressed: bool },
    /// Synthetic pair on physical release; does not expose physical hold duration.
    KnobClick { pressed: bool },
    Rotation { direction: Direction, knob_pressed: bool },
}

/// Firmware components formatted using Studio's version decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirmwareVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    /// Studio omits the fourth component when its byte is zero.
    pub revision: Option<u8>,
}

impl fmt::Display for FirmwareVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(revision) = self.revision { write!(f, ".{revision}")?; }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Reply {
    Heartbeat,
    FirmwareVersion { version: FirmwareVersion, custom_code: [u8; 3], raw_data: [u8; 16] },
    DeviceAddress([u8; 6]),
    /// Opaque flash identifier; no subfield meanings are assumed.
    FlashId([u8; 16]),
    Haptics { enabled: bool, write_response: bool },
    SleepTimer { seconds: u16, write_response: bool },
    /// Only the observed two-fragment serial response is established.
    SerialFragment { index: u8, text: Vec<u8> },
}

/// Unsolicited battery/status report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatteryStatus {
    /// Little-endian voltage reading; consistent with millivolts, not yet measured.
    pub voltage_raw: u16,
    pub battery_percent: u8,
    /// Raw two-bit status; meanings of the nonzero values are not yet established.
    pub low_battery_status: u8,
    pub power_off: bool,
    pub charging: bool,
    /// The firmware flag is independent of battery_percent being 100.
    pub charge_full: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message { Event(Event), Reply(Reply), BatteryStatus(BatteryStatus), Unknown(Packet) }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Heartbeat, ReadHaptics, SetHaptics(bool), ReadSleepTimer, SetSleepTimer(u16), ReadSerial,
    ReadFirmwareVersion, ReadDeviceAddress, ReadFlashId,
    /// Compatibility name for ReadDeviceAddress; this is a query, not authentication.
    ObservedSetup,
    Raw(Packet),
}

impl Command {
    /// UI-tested values are 0, 3600, 7200, 10800 and 14400 seconds.
    /// Other representable values are unverified with hardware.
    pub fn sleep_timer(duration: Duration) -> Result<Self, ProtocolError> {
        if duration.subsec_nanos() != 0 || duration.as_secs() > u64::from(u16::MAX) {
            return Err(ProtocolError::InvalidSleepDuration);
        }
        Ok(Self::SetSleepTimer(duration.as_secs() as u16))
    }

    pub fn packet(self) -> Packet {
        let mut p = [0; PACKET_LEN];
        let prefix: &[u8] = match self {
            Self::Heartbeat => &[6, 1, 0x23, 0, 1],
            Self::ReadHaptics => &[1, 6, 0x40, 1],
            Self::SetHaptics(enabled) => &[1, 6, 0x40, 4, if enabled { 0xff } else { 0 }],
            Self::ReadSleepTimer => &[1, 1, 0x42, 1],
            Self::SetSleepTimer(seconds) => {
                p[..4].copy_from_slice(&[1, 1, 0x42, 2]);
                p[4..6].copy_from_slice(&seconds.to_le_bytes());
                return Packet(p);
            }
            Self::ReadSerial => &[1, 1, 0x0b, 1],
            Self::ReadFirmwareVersion => &[1, 4, 4, 1],
            Self::ReadFlashId => &[1, 4, 0x0b, 1],
            Self::ReadDeviceAddress | Self::ObservedSetup => &[1, 1, 0xfa, 1],
            Self::Raw(packet) => return packet,
        };
        p[..prefix.len()].copy_from_slice(prefix);
        Packet(p)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolError { InvalidLength(usize), UnknownButton(u8), InvalidSleepDuration }

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength(n) => write!(f, "expected 20-byte vendor value, received {n}"),
            Self::UnknownButton(id) => write!(f, "unknown physical button ID {id:#x}"),
            Self::InvalidSleepDuration => f.write_str("sleep duration must be whole seconds in 0..=65535"),
        }
    }
}
impl Error for ProtocolError {}
