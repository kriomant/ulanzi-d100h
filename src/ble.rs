//! BLE I/O with explicit characteristic selection. No unverified handshake is automatic.
use std::{error::Error, fmt, pin::Pin};
use btleplug::api::{CharPropFlags, Characteristic, Peripheral as _, ValueNotification, WriteType};
use btleplug::platform::Peripheral;
use futures_util::{Stream, StreamExt as _};
use crate::{Command, Message, Packet, ProtocolError};

type Notifications = Pin<Box<dyn Stream<Item = ValueNotification> + Send>>;

pub struct Client {
    peripheral: Peripheral,
    write: Characteristic,
    notify: Characteristic,
    notifications: Notifications,
}

impl Client {
    /// The caller connects the peripheral and calls discover_services first.
    /// Selection must refer to the verified vendor write and notify characteristics.
    pub async fn attach(peripheral: Peripheral, write: Characteristic, notify: Characteristic)
        -> Result<Self, BleError>
    {
        if !write.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
            || !notify.properties.contains(CharPropFlags::NOTIFY)
        {
            return Err(BleError::InvalidCharacteristics);
        }
        let discovered = peripheral.characteristics();
        if !discovered.contains(&write) || !discovered.contains(&notify)
            || write.uuid == notify.uuid
        {
            return Err(BleError::InvalidCharacteristics);
        }
        let notifications = peripheral.notifications().await?;
        peripheral.subscribe(&notify).await?;
        Ok(Self { peripheral, write, notify, notifications })
    }

    /// Sends a command. Success means the BLE write succeeded, not that the device
    /// acknowledged it. Read notifications to inspect the corresponding reply.
    pub async fn send(&self, command: Command) -> Result<(), BleError> {
        self.peripheral.write(&self.write, &command.packet().to_wire(), WriteType::WithoutResponse).await?;
        Ok(())
    }

    /// Skips notifications from other UUIDs; preserves unknown vendor messages.
    /// Returns None when the notification stream ends. A BLE disconnect may require
    /// monitoring btleplug CentralEvent separately on platforms with persistent streams.
    pub async fn next_message(&mut self) -> Option<Result<Message, BleError>> {
        while let Some(value) = self.notifications.next().await {
            if value.uuid == self.notify.uuid {
                return Some(Packet::from_wire(&value.value).map(Packet::message).map_err(BleError::Protocol));
            }
        }
        None
    }

    /// Explicit cleanup; dropping Client does not asynchronously unsubscribe/disconnect.
    pub async fn disconnect(&self) -> Result<(), BleError> {
        self.peripheral.disconnect().await?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum BleError { Transport(btleplug::Error), Protocol(ProtocolError), InvalidCharacteristics }
impl From<btleplug::Error> for BleError {
    fn from(error: btleplug::Error) -> Self { Self::Transport(error) }
}
impl fmt::Display for BleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => error.fmt(f),
            Self::Protocol(error) => error.fmt(f),
            Self::InvalidCharacteristics => f.write_str("select discovered vendor write-without-response and notify characteristics with distinct UUIDs"),
        }
    }
}
impl Error for BleError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self { Self::Transport(e) => Some(e), Self::Protocol(e) => Some(e), _ => None }
    }
}
