# Ulanzi D100H / Ulanzi Dial BLE protocol

Reverse-engineered from macOS PacketLogger captures of Ulanzi Studio communicating with one controller. This document consolidates all observations from the investigation. It describes the **observed BLE protocol**, not a complete manufacturer specification. Unresolved fields and untested behavior are called out explicitly.

## 1. Device and scope

| Property | Value / status |
|---|---|
| Hardware identified by user | Ulanzi D100H |
| PacketLogger device name | `Ulanzi Dial` |
| Readable device metadata | Manufacturer `KEHWIN`, model `Dial Lite` |
| Firmware shown by Studio | `1.3.8.1`; binary encoding not yet identified |
| Captured serial bytes | `A0D26I003U3711117` |
| User transcription of Studio serial | `AOD26\|003U3711117`; differs visually from captured digit `0` and uppercase `I` |
| Peer Bluetooth address in initialization trace | `ca:61:f0:83:e3:fb` |
| Transport examined | Bluetooth Low Energy, ATT/GATT |

The USB protocol was not captured: the controller did not enumerate as a usable device in the Windows VM during the initial attempt. USB behavior must not be inferred from BLE findings.

## 2. Transport

| Direction | ATT operation | Observed attribute handle | Data |
|---|---|---|---|
| Studio → controller | Write Command, opcode `0x52` | `0x0050` | Encrypted vendor command |
| Controller → Studio | Handle Value Notification, opcode `0x1b` | `0x004d` | Encrypted vendor response or event |
| Studio → controller | Write Request | `0x004e` | `01 00`, enabling notifications on the vendor channel |

Vendor messages in all captures are **20 bytes**. Attribute handles are observed values for this device/firmware, not a verified UUID-based discovery contract. The exact service and characteristic UUIDs associated with these handles remain unresolved. Strings `0000fff0`, `0000fff1`, and `0000fff2` exist in the app's Bluetooth library, but have not been conclusively mapped to this device's channel.

HCI connection handles changed between captures (`0x004a` and `0x0049`). They are local connection identifiers and must not be hard-coded as device identity.

Other observed attributes include `0x001d` (a battery notification with value `64`, consistent with 100%), `0x0032` (eight zero bytes), and `0x003d` (two zero bytes). These are separate from the encrypted vendor channel. Their complete roles and UUID mapping remain unverified. A negotiated ATT MTU of 247 appears in the initialization trace, although observed vendor values remain 20 bytes.

Wireshark had incomplete discovery context and labeled some vendor handles inconsistently. Treat its inferred characteristic labels as unverified; direction, handle, and actual value bytes are the evidence used here.

## 3. Encryption and wire representation

Vendor packets use the following encryption algorithm:

| Parameter | Value |
|---|---|
| Cipher | Original TEA, not XTEA or XXTEA |
| Rounds | 32 |
| Block size | 8 bytes |
| Word format | Two little-endian unsigned 32-bit words per block |
| Delta | `0x9e3779b9` |
| Key bytes | `ca ba a5 ca 6d 8a 2a bc ba 9e 5a ca ca 8b b8 9b` |
| Key words, little-endian | `caa5baca bc2a8a6d ca5a9eba 9bb88bca` |
| Block processing | Independent blocks; no IV or chaining |

For a 20-byte value, encrypt/decrypt bytes **0–7** and **8–15** independently. Bytes **16–19 remain unchanged**. All arithmetic wraps modulo 2³². Encryption/decryption processes `floor(length / 8)` complete blocks, leaving any trailing bytes untouched.

For one block `(v0, v1)`, encryption is:

```text
sum = 0
repeat 32 times:
    sum += delta
    v0 += ((v1 << 4) + k0) XOR (v1 + sum) XOR ((v1 >> 5) + k1)
    v1 += ((v0 << 4) + k2) XOR (v0 + sum) XOR ((v0 >> 5) + k3)
```

Decryption starts with `sum = 0xc6ef3720`, subtracts the `v1` expression first, then the `v0` expression, and subtracts delta after each round.

### Verified vectors

Each row is the full 20-byte value:

| Meaning | Plaintext | Ciphertext |
|---|---|---|
| Top-left button down | `8b10010100000000000000000000000000000000` | `8e8f5e16ec67281e3890c499a360aaad00000000` |
| Top-left button up | `8b10010000000000000000000000000000000000` | `028db3660224d0c73890c499a360aaad00000000` |
| Periodic host message | `0601230001000000000000000000000000000000` | `24569ef228e145a13890c499a360aaad00000000` |
| Periodic reply | `0601231001000000000000000000000000000000` | `21f45eb71c3587ab3890c499a360aaad00000000` |

The decoder was validated by byte-exact re-encryption of decoded vendor messages. The repeated ciphertext block `38 90 c4 99 a3 60 aa ad` corresponds to an eight-byte zero plaintext block.

Reference implementation: [reference_decoder.py](tests/reference_decoder.py). It operates offline on extracted TSV data and does not connect to or modify the controller.

## 4. Control events

All observed control events have this decrypted format:

| Offset | Length | Meaning |
|---|---|---|
| 0 | 1 | `8b`, observed control-event prefix |
| 1 | 1 | `10`, observed constant |
| 2 | 1 | Control/event ID |
| 3 | 1 | State: `01` down/step, `00` up |
| 4 | 16 | Zero in all observed control events |

Positions below are from the front of the controller, with the Ulanzi logo upright. Photo numbers refer to the user's numbered reference picture.

| ID (hex) | Control | Photo number |
|---|---|---|
| `01` | Top-left button | 3 |
| `02` | Top-middle button | 4 |
| `03` | Top-right button | 5 |
| `04` | Upper-left side button | 2 |
| `05` | Upper-right side button | 6 |
| `06` | Lower-left side button | 1 |
| `07` | Knob click | 10 |
| `08` | Lower-right side button | 7 |
| `09` | Free counterclockwise rotation | 8 |
| `0a` | Free clockwise rotation | 9 |
| `0b` | Counterclockwise rotation with knob pressed | — |
| `0c` | Clockwise rotation with knob pressed | — |

### Physical buttons

IDs `01`–`06` and `08` report independent down (`01`) and up (`00`) transitions. A two-second lower-left hold produced down/up events separated by **2.085 seconds**. No separate long-press or repeat control message was observed in that experiment. A client can measure hold duration itself; this does not establish that every button or every configuration lacks additional long-press behavior.

### Knob click

The knob differs from the physical buttons:

- Holding it while stopping the capture before release produced **no ID `07` event**.
- Starting a capture with it already held, then releasing it, produced `8b 10 07 01` followed by `8b 10 07 00` **1 ms later**.
- Therefore the observed knob click is a synthetic down/up pair emitted **on physical release**. Its down/up interval does not represent the physical hold duration.
- After rotating while pressed, no ID `07` click pair appeared on release in either held-rotation experiment. This supports release-click suppression after held rotation; boundary cases remain untested.

Handle the pair as one click. Do not use ID `07` as a physical knob-held state.

### Rotation

Each observed detent generated one event with state `01`. Two clicks in each direction consistently produced exactly two events. No rotation-up events were observed.

| Condition | Counterclockwise | Clockwise |
|---|---|---|
| Knob not pressed | `8b 10 09 01` | `8b 10 0a 01` |
| Knob pressed | `8b 10 0b 01` | `8b 10 0c 01` |

Held rotation is encoded directly in the event ID; no prior knob-down message is necessary. Fast rotation, acceleration, and any aggregation behavior have not been tested.

### Simultaneous presses and button modifiers

Independent button transitions survived the tested overlaps:

| Test | Confirmed result |
|---|---|
| Lower-left + upper-left | Down/down/up/up; overlap lasted 1.335 s |
| Three top buttons | All three downs and releases preserved; middle released first |
| Upper-left + upper-right | Independent overlapping states |
| Lower-left + top-left | Independent overlapping states |
| Rotation with lower-left held | IDs `09`/`0a`; button remains down until separate up |
| Rotation with top-middle held | IDs `09`/`0a`; button state preserved |
| Rotation with lower-left + upper-right held | IDs `09`/`0a`; both button states preserved |
| Knob-held rotation + lower-left held | IDs `0b`/`0c`; side-button state preserved; no release click |

A client can implement “second button pressed while first is held” and button-modified rotation by maintaining a set of pressed button IDs. At least **three-button rollover** is confirmed for the top row. Every possible pair and maximum rollover are not yet tested.

Suggested event processing:

```text
physical button down: add ID to pressed set; dispatch down with current set
physical button up:   dispatch up; remove ID from pressed set
ID 07, state 01:      dispatch one knob click
ID 07, state 00:      finish/ignore the synthetic click release
ID 09/0a:            dispatch ordinary rotation with current pressed set
ID 0b/0c:            dispatch knob-held rotation with current pressed set
connection loss:     clear pressed set
```

## 5. Settings commands

All examples below are **plaintext prefixes**. Pad them with zero bytes to 20 bytes, then apply the encryption described above and send via the vendor write characteristic.

### Haptic feedback

| Operation | Plaintext prefix |
|---|---|
| Read current value | `01 06 40 01` |
| Read response | `01 06 40 11 VALUE` |
| Set value | `01 06 40 04 VALUE` |
| Set response | `01 06 40 14 VALUE` |

`VALUE = 00` disables haptics; `VALUE = ff` enables them. Both writes and matching replies were captured. The initialization read returned `ff`.

### Sleep timer

| Operation | Plaintext prefix |
|---|---|
| Read current value | `01 01 42 01` |
| Read response | `01 01 42 11 LO HI` |
| Set value | `01 01 42 02 LO HI` |
| Set response | `01 01 42 12 LO HI` |

`LO HI` is a little-endian 16-bit duration in seconds:

| Studio option | Seconds | Bytes |
|---|---:|---|
| Close / disabled | 0 | `00 00` |
| 1 hour | 3600 | `10 0e` |
| 2 hours | 7200 | `20 1c` |
| 3 hours | 10800 | `30 2a` |
| 4 hours | 14400 | `40 38` |

All five values were captured. Arbitrary durations outside these UI options have not been tested. The observed initialization read returned `20 1c` (two hours).

All seven settings writes received matching responses about **20–30 ms later**, echoing the requested value. This confirms the exchange, but persistence across power cycles and actual sleep behavior were not tested. The last writes restored haptics enabled and sleep to two hours.

## 6. Initialization, identity, and periodic traffic

The initial trace includes a successful connection, notification setup, startup commands, and subsequent periodic traffic. This is an observed sequence, not yet a verified minimum handshake for an independent client.

### Readable metadata

Studio reads these attributes in the initial trace:

| Handle | Captured value |
|---|---|
| `0x000a` | `KEHWIN` |
| `0x000c` | `Dial Lite` |
| `0x000e` | `20250600` |
| `0x0010` | `1.0.0` |
| `0x0012` | `5.3` |
| `0x0014` | `V0.0.1` |
| `0x0016` | `kehwin` |
| `0x0018` | `IEEE` |
| `0x001a` | `02 f1 ff 82 00 00 00` |

These strings must not be substituted for Studio's reported firmware `1.3.8.1` without establishing their actual field roles.

### Startup vendor exchanges

Rows show meaningful observed prefixes or full payload bytes after the four-byte prefix. Where shorter than 20 bytes, the remaining bytes are zero.

| Host prefix | Reply / data | Interpretation |
|---|---|---|
| `01 01 fa 01` | `01 01 fa 11 ca 61 f0 83 e3 fb` | Setup/identity exchange; reply contains captured peer address; exact authentication role unresolved |
| `01 04 0b 01` | `01 04 0b 11 42 50 31 5a 35 38 38 02 00 25 c6 43 56 03 e2 78` | Unresolved; includes ASCII `BP1Z588` |
| `01 01 0b 01` | Two fragment forms below | Serial-number query |
| `01 01 02 01` | `01 01 02 11 78 10 64` | Unresolved |
| `01 06 40 01` | `01 06 40 11 ff` | Read haptics enabled |
| `01 01 42 01` | `01 01 42 11 20 1c` | Read sleep timer, two hours |
| `01 04 04 01` | `01 04 04 11 40 00 37 28 28 01 01 03 08 26 04 08 18 00 01 00` | Unresolved; firmware encoding may be here but is not confirmed |

`01 01 fa 01` was transmitted repeatedly during startup, at roughly 200 ms spacing. The trace also includes duplicate replies and repeated reads. Required retries, startup ordering, timeout rules, and success criteria remain unresolved.

### Serial fragmentation

The serial query `01 01 0b 01` produced:

```text
01 01 0b 11 0a 00 41 30 44 32 36 49 30 30 33 55 00 00 00 00
01 01 0b 11 07 01 33 37 31 31 31 31 37 00 00 00 00 00 00 00
```

The data supports this interpretation: byte 4 is fragment text length (`0a` = 10; `07` = 7), byte 5 is fragment index (`00`, `01`), and text starts at byte 6. Concatenation yields `A0D26I003U3711117`. This is established for this two-fragment response; broader fragmentation rules are not tested.

### Online mode and periodic exchange

After initialization, approximately once per second:

```text
Host:       06 01 23 00 01 00 ...
Controller: 06 01 23 10 01 00 ...
```

**Hardware-confirmed:** with Studio fully quit, the independent Rust example
subscribed to the vendor notify characteristic and sent this heartbeat immediately
and once per second. Vendor control events became available and factory HID
keyboard/media actions stopped. No other startup commands were sent. Thus the
heartbeat is sufficient to enable online operation on the tested firmware 1.3.8.1.

Subscription alone had left factory HID actions active and some controls absent
from the vendor event stream. The timeout and fallback behavior after heartbeats
stop remain unmeasured. The initial Studio capture shows heartbeat traffic both
before and after reconnection.

### Additional unsolicited message

Notifications beginning `0b 7b XX 10 00 64` were observed, where `XX` varied (`78`, `76`, `72`, `71` across experiments). A separate battery attribute also reported `64`. The vendor message may contain status/battery information, but the meaning of `XX` and the full message is not confirmed.

## 7. Implementation boundaries

A practical receiver can decrypt the vendor notifications, parse control events, track overlapping physical buttons, and distinguish free versus knob-held rotation using the confirmed fields above. The settings command bytes are captured and mapped, but they have not yet been replayed from an independent client.

Remaining work:

- Confirm GATT service/characteristic UUIDs and a discovery procedure.
- Check whether heartbeat-only initialization also works after a power cycle; characterize any authentication requirements across other devices/firmware.
- Identify firmware/version and unresolved startup/status fields.
- Measure heartbeat expiry/fallback behavior, disconnect handling, and any transaction identifiers.
- Test maximum simultaneous presses, additional pairs, fast rotation, and knob edge cases.
- Test settings persistence and arbitrary sleep values, if desired.
- Investigate USB separately.

No checksum or transaction counter has been identified in the mapped messages. That is an observation, not proof that neither exists elsewhere in the protocol. Meanings of general command-prefix bytes must not be extrapolated beyond the captured commands.

## 8. Evidence and artifacts

Decoded tables remain in the original investigation workspace. The original offline
decoder is preserved in `tests/reference_decoder.py`.
