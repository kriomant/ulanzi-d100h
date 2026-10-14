# Python bindings

PyO3 bindings for the Rust protocol and BLE client, supporting Python 3.10+.
Import the package as `ulanzi_d100h`.

From the repository root, install into a virtual environment (Rust is required):

```sh
python3 -m venv .venv
.venv/bin/python -m pip install ./ulanzi-d100h-python
.venv/bin/python -m unittest discover -s ulanzi-d100h-python/tests
```

## Protocol

```python
from ulanzi_d100h import Command, Packet

wire = Command.set_haptics(True).packet().to_wire()
plaintext = Packet.from_wire(wire).plaintext
# For incoming vendor notifications:
# message = Packet.from_wire(notification_bytes).message()
```

`Packet(plaintext)` and `Packet.from_wire(wire)` require exactly 20 bytes and
raise `ValueError` otherwise. `plaintext` and `to_wire()` return `bytes`.

Command factories: `heartbeat()`, `read_haptics()`, `set_haptics(enabled)`,
`read_sleep_timer()`, `set_sleep_timer(seconds)`, `sleep_timer(duration)`,
`read_serial()`, `read_firmware_version()`, `read_device_address()`,
`read_flash_id()`, `observed_setup()`, and `raw(packet)`. Sleep seconds must be
an integer in 0–65535; out-of-range values raise `OverflowError`.
`sleep_timer(duration)` accepts a `datetime.timedelta` and raises `ValueError`
unless it represents whole seconds in 0–65535. `observed_setup()` is a
compatibility alias for `read_device_address()`; it is never sent automatically.

Decoded messages are dictionaries:

| `type` | Additional keys |
|---|---|
| `button` | `button` (numeric Rust button ID), `pressed` |
| `knob_click` | `pressed` |
| `rotation` | `direction` (`clockwise` or `counterclockwise`), `knob_pressed` |
| `heartbeat` | None |
| `firmware_version` | `version` (formatted string), `major`, `minor`, `patch`, `revision` (integer or `None`), `custom_code` (3 bytes), `raw_data` (16 bytes) |
| `device_address` | `address` (6 bytes in wire order) |
| `flash_id` | `data` (16 opaque bytes) |
| `haptics` | `enabled`, `write_response` |
| `sleep_timer` | `seconds`, `write_response` |
| `serial_fragment` | `index`, `text` (bytes) |
| `battery_status` | `voltage_raw`, `battery_percent`, `low_battery_status`, `power_off`, `charging`, `charge_full` |
| `unknown` | `plaintext` (all 20 bytes) |

## BLE

Connect the Dial in the OS Bluetooth settings and close Ulanzi Studio first.
`await discover()` returns `(device_id, name)` tuples for connected peripherals
exposing a vendor service; it does not perform an advertisement scan.

```python
import asyncio
from contextlib import suppress
from ulanzi_d100h import Client, Command, discover

async def heartbeat(client):
    while True:
        await client.send(Command.heartbeat())
        await asyncio.sleep(1)

async def main():
    print(await discover())
    client = await Client.connect()  # Or Client.connect(device_id="...")
    beats = asyncio.create_task(heartbeat(client))
    try:
        while (message := await client.next_message()) is not None:
            print(message)
    finally:
        beats.cancel()
        try:
            with suppress(asyncio.CancelledError):
                await beats
        finally:
            await client.disconnect()

asyncio.run(main())
```

Connection requires exactly one matching device and one unambiguous vendor
write/notify pair. `connect()` connects, discovers services, and calls the Rust
client's `attach()` with the selected characteristics. BLE failures raise
`RuntimeError`. Heartbeats must be sent
immediately and every second to enable vendor control events; the client does
not send them automatically. Sending and receiving can run concurrently.
Always await `disconnect()` for explicit cleanup. `next_message()` returns
`None` when its notification stream ends, though some platforms may keep that
stream open after a physical disconnect.

The BLE backend has the same platform requirements as btleplug. Linux builds
require the D-Bus development libraries; macOS may request Bluetooth permission.
