//! Read-only discovery: connects and prints GATT UUIDs, but sends no vendor commands.
use std::{error::Error, time::Duration};
use btleplug::api::{Central as _, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let manager = Manager::new().await?;
    for adapter in manager.adapters().await? {
        adapter.start_scan(ScanFilter::default()).await?;
        tokio::time::sleep(Duration::from_secs(5)).await;
        adapter.stop_scan().await?;
        for peripheral in adapter.peripherals().await? {
            let properties = peripheral.properties().await?;
            let name = properties.as_ref().and_then(|p| p.local_name.as_deref()).unwrap_or("");
            if !name.to_ascii_lowercase().contains("ulanzi") { continue; }
            println!("{name}: {:?}", peripheral.id());
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
    Ok(())
}
