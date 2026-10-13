# ulanzi-d100h

Rust library for the reverse-engineered Ulanzi D100H / Ulanzi Dial BLE protocol,
verified against captures from firmware **1.3.8.1**.

- Dependency-free 20-byte packet codec with TEA encryption/decryption.
- Typed button, knob click, rotation, and held-knob rotation events.
- Haptics, sleep timer, serial query, and observed heartbeat commands.
- Optional asynchronous BLE client using btleplug.

See [PROTOCOL.md](PROTOCOL.md) for the complete evidence and limitations.

## Decode notifications

```rust
use ulanzi_d100h::{Message, Packet};

fn receive(value: &[u8]) {
    match Packet::from_wire(value).map(Packet::message) {
        Ok(Message::Event(event)) => {
            println!("{event:?}");
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

After subscribing, send `Command::Heartbeat` immediately and once per second.
This was confirmed with Studio fully quit: vendor control events became available
and the factory keyboard/media actions stopped. No other startup commands were
needed in this experiment on firmware 1.3.8.1. Characteristic UUID mapping still
requires confirmation. A successful BLE write does not imply acknowledgment.

`Client` itself does not schedule heartbeats; the `events` example does. The
heartbeat expiry time and behavior after stopping it remain unmeasured. There is
no automatic timeout, reconnect, or response correlation. Monitor btleplug
central disconnect events as appropriate for the platform. Call Client::disconnect
for explicit cleanup.

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

## Print live events

Automatically select the write/notify channels:

```sh
cargo run --features ble --example events
```

The example looks for a unique pair of distinct write-without-response and notify
characteristics in the same Ulanzi Studio service (`1971` or `2013`) and prints the
selected UUIDs. This is a property-based heuristic; the vendor UUID mapping remains
unverified. If no pair or multiple pairs match, it reports an error. You can override
selection with `-- <write-uuid> <notify-uuid> [device-id]`.

The example connects to a Dial already connected in Bluetooth settings and prints
decoded events, replies, and unknown messages until disconnected or stopped with
Ctrl-C. If multiple devices are available, append `-- <device-id>` using the ID
printed by `discover`.
Close Studio first. The example sends the heartbeat immediately and every second
to enable online control events and suppress factory HID actions.

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
Knob hold state cannot be inferred from that pair; held rotation has its own
event IDs.

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

## Standalone HID behavior and online mode

The `events` example now sends the captured Studio heartbeat immediately and
once per second. Run with `--passive` to retain notification-only behavior:

```sh
cargo run --features ble --example events
cargo run --features ble --example events -- --passive
```

Confirmed by the user on firmware 1.3.8.1 with Studio fully quit: sending the
heartbeat alone enables vendor control events and suppresses factory HID actions.
The example performs no other startup exchanges. This confirms sufficiency for
this device/session; the timeout after stopping heartbeats is not yet measured.

The Homebrew project's observation that custom profiles do not persist on the
device does not establish that vendor commands cannot switch its input mode.
