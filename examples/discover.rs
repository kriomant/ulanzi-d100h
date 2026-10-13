//! Read-only discovery: connects and prints GATT UUIDs, but sends no vendor commands.
use std::error::Error;
use btleplug::api::{Central as _, Manager as _, Peripheral as _, RetrievePeripheralsOptions};
use btleplug::platform::Manager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let manager = Manager::new().await?;
    let connected_only = std::env::args().any(|arg| arg == "--connected");
    let mut found = false;
    for adapter in manager.adapters().await? {
        // Match either service used by Ulanzi Studio, even without advertisements.
        let connected = adapter.retrieve_peripherals(RetrievePeripheralsOptions {
            services: Some(vec![
                "00001971-0000-1000-8000-00805f9b34fb".parse()?,
                "00002013-0000-1000-8000-00805f9b34fb".parse()?,
            ]),
            ..Default::default()
        }).await?;
        for peripheral in &connected {
            let properties = peripheral.properties().await?;
            let name = properties.as_ref().and_then(|p| p.local_name.as_deref())
                .unwrap_or("<unnamed peripheral>");
            println!("connected {name}: id={}", peripheral.id());
        }
        if connected_only {
            if connected.is_empty() {
                println!("No connected peripherals exposing either Ulanzi Studio service were returned.");
            }
            continue;
        }
        for peripheral in connected {
            let properties = peripheral.properties().await?;
            let name = properties.as_ref().and_then(|p| p.local_name.as_deref());
            let advertisement_name = properties.as_ref().and_then(|p| p.advertisement_name.as_deref());
            let label = name.or(advertisement_name).unwrap_or("<unnamed peripheral>");
            found = true;
            println!("discovering target {label}");
            peripheral.connect().await?;
            let result = peripheral.discover_services().await;
            if result.is_ok() {
                for service in peripheral.services() {
                    println!("service {}", service.uuid);
                    for c in service.characteristics {
                        println!("  {} {:?}", c.uuid, c.properties);
                    }
                }
            }
            peripheral.disconnect().await?;
            result?;
        }
    }
    if !found && !connected_only {
        eprintln!("No connected device exposing either Ulanzi Studio service was found. Connect the Dial in macOS Bluetooth settings first.");
    }
    Ok(())
}
