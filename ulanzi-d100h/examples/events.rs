//! Connect to a Dial and print decoded vendor notifications.
use std::{error::Error, time::Duration};
use btleplug::api::{Central as _, CentralEvent, CharPropFlags, Manager as _, Peripheral as _, RetrievePeripheralsOptions};
use btleplug::platform::Manager;
use futures_util::StreamExt as _;
use ulanzi_d100h::{ble::Client, Command, Message, Reply};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 1 && matches!(args[0].as_str(), "-h" | "--help") {
        println!("Usage: events [device-id]\n       events <write-uuid> <notify-uuid> [device-id]");
        return Ok(());
    }
    if args.len() > 3 {
        return Err("Usage: events [device-id] | events <write-uuid> <notify-uuid> [device-id]".into());
    }
    let device_id = if args.len() == 1 { args.first() } else { args.get(2) };
    let manager = Manager::new().await?;
    let mut candidates = Vec::new();
    for adapter in manager.adapters().await? {
        let peripherals = adapter.retrieve_peripherals(RetrievePeripheralsOptions {
            services: Some(vec![
                "00001971-0000-1000-8000-00805f9b34fb".parse()?,
                "00002013-0000-1000-8000-00805f9b34fb".parse()?,
            ]),
            ..Default::default()
        }).await?;
        for peripheral in peripherals {
            if device_id.is_none_or(|id| *id == peripheral.id().to_string()) {
                candidates.push((adapter.clone(), peripheral));
            }
        }
    }
    if candidates.is_empty() {
        return Err("No matching connected Dial found. Connect it in Bluetooth settings first.".into());
    }
    if candidates.len() != 1 {
        return Err("Multiple devices found. Specify a device-id from the discover example.".into());
    }
    let (adapter, peripheral) = candidates.remove(0);
    let id = peripheral.id();
    let mut central_events = adapter.events().await?;
    peripheral.connect().await?;
    let result: Result<(), Box<dyn Error>> = async {
        peripheral.discover_services().await?;
        let characteristics = peripheral.characteristics();
        let (write, notify) = if args.len() >= 2 {
            let write = characteristics.iter().find(|c| c.uuid.to_string().eq_ignore_ascii_case(&args[0]))
                .ok_or("Write characteristic not found")?.clone();
            let notify = characteristics.iter().find(|c| c.uuid.to_string().eq_ignore_ascii_case(&args[1]))
                .ok_or("Notify characteristic not found")?.clone();
            (write, notify)
        } else {
            // Restrict the heuristic to services used by Studio, excluding battery/HID.
            let mut pairs = Vec::new();
            for service in peripheral.services() {
                if !matches!(service.uuid.to_string().as_str(),
                    "00001971-0000-1000-8000-00805f9b34fb" |
                    "00002013-0000-1000-8000-00805f9b34fb") {
                    continue;
                }
                for write in &service.characteristics {
                    if !write.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE) {
                        continue;
                    }
                    for notify in &service.characteristics {
                        if notify.properties.contains(CharPropFlags::NOTIFY) && notify.uuid != write.uuid {
                            pairs.push((write.clone(), notify.clone()));
                        }
                    }
                }
            }
            if pairs.len() != 1 {
                for (write, notify) in &pairs {
                    eprintln!("Candidate: service={} write={} notify={}", write.service_uuid, write.uuid, notify.uuid);
                }
                return Err(format!(
                    "Found {} candidate channel pairs; expected exactly one. Run the discover example and supply explicit write/notify UUIDs.",
                    pairs.len(),
                ).into());
            }
            pairs.remove(0)
        };
        println!("Using service={} write={} notify={}", write.service_uuid, write.uuid, notify.uuid);
        let mut client = Client::attach(peripheral.clone(), write, notify).await?;
        client.send(Command::Heartbeat).await?;
        let mut heartbeat = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_secs(1),
            Duration::from_secs(1),
        );
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        println!("Listening to {id}. Press Ctrl-C to stop.");
        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    client.send(Command::Heartbeat).await?;
                },
                message = client.next_message() => match message {
                    Some(Ok(Message::Reply(Reply::Heartbeat))) => {},
                    Some(Ok(message)) => println!("{message:?}"),
                    Some(Err(error)) => eprintln!("Notification error: {error}"),
                    None => break,
                },
                Some(event) = central_events.next() => {
                    if matches!(event, CentralEvent::DeviceDisconnected(device) if device == id) {
                        eprintln!("Device disconnected.");
                        break;
                    }
                }
            }
        }
        Ok(())
    }.await;
    if peripheral.is_connected().await? {
        peripheral.disconnect().await?;
    }
    result
}
