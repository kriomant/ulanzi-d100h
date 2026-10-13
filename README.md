# ulanzi-d100h

Rust library for the reverse-engineered Ulanzi D100H / Ulanzi Dial BLE protocol,
verified against captures from firmware **1.3.8.1**.

- Dependency-free 20-byte packet codec with TEA encryption/decryption.
- Typed button, knob click, rotation, and held-knob rotation events.
- Overlapping button tracking and contextual rotation actions.
- Haptics, sleep timer, serial query, and observed heartbeat commands.
- Optional asynchronous BLE client using btleplug.

See [PROTOCOL.md](PROTOCOL.md) for the complete evidence and limitations.

## Decode notifications

```rust
use ulanzi_d100h::{ButtonState, Message, Packet};

fn receive(value: &[u8], state: &mut ButtonState) {
    match Packet::from_wire(value).map(Packet::message) {
        Ok(Message::Event(event)) => {
            if let Some(action) = state.apply(event) {
                println!("{action:?}");
            }
        }
        other => println!("{other:?}"),
    }
}
```

Feed only the vendor notify characteristic into this decoder. Other GATT values
have different formats. Unknown plaintext messages are preserved.

## BLE communication

Enable the `ble` feature. Use btleplug to scan, connect, and discover services,
then explicitly select the controller's vendor write-without-response and notify
characteristics and pass them to `ble::Client::attach`.

```ignore
let mut client = ulanzi_d100h::ble::Client::attach(peripheral, write, notify).await?;
client.send(ulanzi_d100h::Command::ReadHaptics).await?;
while let Some(message) = client.next_message().await {
    println!("{:?}", message?);
}
```

The code above assumes the device session has been initialized. **The minimum
startup/authentication sequence and characteristic UUID mapping are not verified.**
Consequently, attach subscribes but does not send a guessed handshake. The raw
command API supports experiments based on the observed sequence in PROTOCOL.md.
A successful BLE write does not imply application-level acknowledgment.

Studio sent `Command::Heartbeat` approximately once per second. Its necessity is
unverified; callers can schedule it explicitly and consume replies. There is no
automatic heartbeat, timeout, reconnect, or response correlation. Monitor btleplug
central disconnect events as appropriate for the platform and clear ButtonState
on disconnect. Call Client::disconnect for explicit cleanup.

List discovered UUIDs without sending vendor writes:

```sh
cargo run --features ble --example discover
```

Discovery searches connected peripherals exposing either service used by Ulanzi
Studio: `00001971-0000-1000-8000-00805f9b34fb` or
`00002013-0000-1000-8000-00805f9b34fb`. It does not scan or filter by device name.
Connect the Dial in macOS Bluetooth settings first.
To print just their names and CoreBluetooth IDs without
connecting or discovering GATT services:

```sh
cargo run --features ble --example discover -- --connected
```

Close Studio before connecting independently. macOS may request Bluetooth
permission for the terminal or application running the example. Platform Bluetooth
requirements are inherited from btleplug (Linux additionally needs D-Bus development
libraries). Discovery output still requires identification of the correct vendor
characteristics; the example does not guess.

## Commands

```rust
use std::time::Duration;
use ulanzi_d100h::Command;
let wire = Command::SetHaptics(true).packet().to_wire();
let sleep = Command::sleep_timer(Duration::from_secs(7200)).unwrap();
```

`SetSleepTimer` accepts a u16 duration in seconds. Only 0, 3600, 7200, 10800, and
14400 were tested via Studio. Duration conversion rejects fractional/overflowing
values. Settings writes have been decoded from captures, not replayed independently.
Serial replies expose raw fragments; automatic serial assembly is intentionally
left to callers because general fragmentation completion rules are unverified.

The knob click arrives as a synthetic down/up pair **on physical release**.
ButtonState emits one click for its down and ignores the synthetic up. Knob hold
state cannot be inferred from that pair; held rotation has its own event IDs.

## Validation

```sh
cargo test
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

Regression fixtures contain captured ciphertext/plaintext pairs, including device
identity data. Codec round trips are checked against those captured values rather
than only internally generated packets. No live-device behavior is claimed by these
tests. No firmware upgrade or USB transport is implemented.
