# Ulanzi D100H / Ulanzi Dial BLE protocol

Reverse-engineered from macOS PacketLogger captures of Ulanzi Studio communicating with one controller. This document consolidates all observations from the investigation. It describes the **observed BLE protocol**, not a complete manufacturer specification. Unresolved fields and untested behavior are called out explicitly.

## 1. Device and scope

| Property | Value / status |
|---|---|
| Hardware identified by user | Ulanzi D100H |
| PacketLogger device name | `Ulanzi Dial` |
| Readable device metadata | Manufacturer `KEHWIN`, model `Dial Lite` |
| Firmware shown by Studio | `1.3.8.1`; decoded from vendor version response |
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

Vendor messages in all captures are **20 bytes**. Live GATT discovery and a
successful independent client session confirmed these UUIDs on the tested Dial:

| Role | UUID |
|---|---|
| Vendor service | `00001971-0000-1000-8000-00805f9b34fb` |
| Write without response | `0000fd01-0000-1000-8000-00805f9b34fb` |
| Notify | `0000fd00-0000-1000-8000-00805f9b34fb` |

Use these UUIDs for discovery rather than hard-coding the captured attribute
handles. Strings `0000fff0`, `0000fff1`, and `0000fff2` in the other Bluetooth
library do not identify this Dial's observed vendor channel.

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
- After rotating while pressed, no ID `07` click pair appeared on release. A later experiment confirmed that one reported step in either direction suffices to suppress the release click; sub-step movement remains untested.

Handle the pair as one click. Do not use ID `07` as a physical knob-held state.

### Rotation

Each observed detent generated one event with state `01`. Two clicks in each direction consistently produced exactly two events. No rotation-up events were observed.

| Condition | Counterclockwise | Clockwise |
|---|---|---|
| Knob not pressed | `8b 10 09 01` | `8b 10 0a 01` |
| Knob pressed | `8b 10 0b 01` | `8b 10 0c 01` |

Held rotation is encoded directly in the event ID; no prior knob-down message is necessary. A later fast-rotation capture retained the same single-event packet format with no count or acceleration field; event loss and the maximum supported rate remain unmeasured.

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

A client can implement “second button pressed while first is held” and button-modified rotation by maintaining a set of pressed button IDs. Full **seven-button simultaneous tracking** is confirmed by the later all-buttons experiment. Every possible combination with knob press/rotation is not yet tested.

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

The initial trace includes Studio's connection, notification setup, startup
queries, and periodic traffic. A subsequent independent-client test after a power
cycle confirmed that the additional Studio startup queries are unnecessary for
control events on this device/firmware.

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
| `01 01 fa 01` | `01 01 fa 11 ca 61 f0 83 e3 fb` | MAC-address query (identified by `getDeviceMacAddressMessage`) |
| `01 04 0b 01` | `01 04 0b 11 42 50 31 5a 35 38 38 02 00 25 c6 43 56 03 e2 78` | Flash-ID query (`getDeviceFlashIdMessage`); opaque identifier includes ASCII `BP1Z588` |
| `01 01 0b 01` | Two fragment forms below | Serial-number query |
| `01 01 02 01` | `01 01 02 11 78 10 64` | Unresolved |
| `01 06 40 01` | `01 06 40 11 ff` | Read haptics enabled |
| `01 01 42 01` | `01 01 42 11 20 1c` | Read sleep timer, two hours |
| `01 04 04 01` | `01 04 04 11 40 00 37 28 28 01 01 03 08 26 04 08 18 00 01 00` | Firmware version response; fields decoded below |

`01 01 fa 01` was transmitted repeatedly during startup, at roughly 200 ms spacing. The trace also includes duplicate replies and repeated reads. The command is a MAC-address query, not an authentication request. Its repetition is part of Studio's discovery flow; independent online operation does not require it.

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

**Power-cycle confirmation:** Studio was quit, the Dial was powered off for
10 seconds and powered on, then connected through macOS Bluetooth settings.
The independent client subscribed and sent only heartbeats. The user confirmed
normal operation for all requested controls with factory HID actions suppressed.
No prior Studio session after the power cycle or additional application-level
setup/authentication exchange was required. This used an already paired device;
first-time pairing and other firmware versions remain outside this result.

Subscription alone had left factory HID actions active and some controls absent
from the vendor event stream. **Expiry measurement:** the client stopped heartbeats while keeping BLE connected
and notifications subscribed. Continuous clockwise rotation produced its last
vendor event at 3.007 seconds after the final host heartbeat write. The user
continued rotating and observed system volume changing afterwards. This confirms
fallback to standalone HID operation at approximately **3 seconds**, even during
continuous input. BLE remained connected through at least 8.001 seconds.
The measurement is approximate: it timestamps host write completion and received
events, not the exact instant of device mode transition. Send heartbeats every
second to maintain online operation; input alone did not keep it online. The initial Studio capture shows heartbeat traffic both
before and after reconnection.


### Heartbeat payload 00 experiment

After five seconds of online operation, the independent client sent a fresh
normal heartbeat (`06 01 23 00 01 ...`), followed by the same packet with
byte 4 set to `00` (`06 01 23 00 00 ...`). It then stopped all writes while
keeping BLE connected. The user continued rotating and observed system volume
changing approximately three seconds later, consistent with ordinary expiry.
Payload `00` therefore did not produce an immediate return to standalone mode
on the tested firmware. This does not establish whether the packet was ignored,
acknowledged, or affected other state.

**Repeated-00 experiment:** after establishing online operation, the client
switched to sending payload `00` once per second. The user reported vendor events
continuing for at least 30 seconds after the switch, beyond the approximately
three-second expiry following the last payload-01 packet. Thus repeated payload-00
packets sustain vendor event delivery. The user also confirmed system volume did
not change while rotating, so repeated payload-00 packets maintain online mode
with factory HID volume actions suppressed. Both payload values 00 and 01 work as
keepalives on this firmware. Their exact distinction and other values remain
unknown; 00 is not an immediate disable command. Continue using Studio's observed
payload 01 for normal operation.

The repeated-00 run also produced replies with full plaintext
`06 01 23 10 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00`.
This identifies the payload-00 heartbeat response. The library recognizes both
observed reply payloads (00 and 01) as `Reply::Heartbeat`, so the events example
suppresses both. Exact reply cadence remains unmeasured.

### Additional unsolicited message

`0b 7b` identifies a battery/status notification.

| Offset | Meaning |
|---|---|
| 0–1 | `0b 7b`, battery notification prefix |
| 2–3 | Little-endian unsigned 16-bit voltage value |
| 4, bits 0–1 | Low-battery status (raw two-bit value) |
| 4, bit 2 | Power-off flag |
| 4, bit 3 | Charging flag |
| 4, bit 4 | Charge-full flag |
| 5 | Battery percentage |

Example: `0b 7b 22 10 00 64` means voltage value `0x1022` = 4130,
battery 100%, charging false, charge-full false, low-battery status 0,
and power-off false. The voltage value is consistent with millivolts (4.130 V),
though the unit has not been independently measured. Earlier changing bytes
`78`, `76`, `72`, `71` were the low byte of the voltage, not an event ID.
The user observed this report alongside a button event after inactivity;
the precise reporting trigger/interval remains unverified.

## 7. Implementation boundaries

A practical receiver can decrypt the vendor notifications, parse control events, track overlapping physical buttons, and distinguish free versus knob-held rotation using the confirmed fields above. Settings reads and haptics writes have been exercised from the independent client. Both haptics and sleep-timer writes and persistence across a power cycle are confirmed.

Remaining work:

- Check the confirmed vendor UUIDs across other devices/firmware.
- Characterize first-time pairing and any authentication requirements across other devices/firmware.
- Identify remaining version-payload fields and other unresolved startup/status fields.
- Test other-platform disconnect behavior, the distinction (if any) between heartbeat payloads 00 and 01, and any transaction identifiers. Unsubscription while heartbeats continue is confirmed below.
- Measure rotation loss/maximal rate if physical detent counts become available; test remaining knob/button combination edge cases. Fast packet format and all-seven-button tracking are confirmed.
- Test actual sleep timing and arbitrary sleep values, if desired.
- Test whether custom standalone presets affect the observed offline vendor IDs.
- Investigate USB separately.

No checksum or transaction counter has been identified in the mapped messages. That is an observation, not proof that neither exists elsewhere in the protocol. Meanings of general command-prefix bytes must not be extrapolated beyond the captured commands.

## 8. Evidence and artifacts

Decoded tables remain in the original investigation workspace. The original offline
decoder is preserved in `tests/reference_decoder.py`.

## Firmware version decoding

Static inspection identifies `01 04 04 01` as `getDeviceVersionMessage` and
`01 04 04 11` as its response. Offsets below refer to the full decrypted packet:

| Offset | Meaning | Captured value |
|---|---|---|
| 4–6 | Custom code, formatted as three two-digit uppercase hex bytes | `400037` |
| 10 | Firmware major | 1 |
| 11 | Firmware minor | 3 |
| 12 | Firmware patch | 8 |
| 18 | Optional fourth firmware component; omitted when zero | 1 |

Studio uses `%d.%d.%d.%d` for nonzero offset 18 and `%d.%d.%d` otherwise.
This produces **1.3.8.1**, matching the user's Studio display. Other bytes in
this version payload remain uninterpreted and are retained as raw data by the
library. Firmware decoding is established from static inspection, the original capture,
and the later independent query experiment.
The library adds ReadFirmwareVersion, ReadDeviceAddress, and ReadFlashId commands
and typed replies. ObservedSetup remains a compatibility name for ReadDeviceAddress.


### Battery power-change experiment

With Studio quit and normal heartbeats running, the user performed an unplugged
idle/button phase, connected USB power, then disconnected USB power. The logged
status reports were:

| Phase | Voltage raw | Battery % | Charging | Charge full |
|---|---:|---:|---|---|
| Unplugged, report adjacent to first button press | 4125 | 100 | false | false |
| USB connected, report before the next button press | 4124 | 100 | true | false |
| USB disconnected, report before the next button press | 4136 | 100 | false | false |

Low-battery status stayed 0 and power-off stayed false in all three reports.
This confirms the charging flag follows USB power/charging state in this experiment
and that reports can arrive following power changes without a button press.
The first report appeared between button down and up after the idle phase,
consistent with an activity-triggered refresh; that alone does not establish
its exact trigger. The output has no timestamps, so reporting latency and periodic
interval cannot be measured. All samples were at 100%, yet charge-full remained
false, confirming that this flag is separate from the percentage. The meaning of
charge-full=true and nonzero low-battery values still needs observation. Voltage
units remain consistent with millivolts but have not been independently measured.


### Haptics persistence across a power cycle

The user completed the requested experiment with Studio quit:

| Phase | Haptics | Sleep timer |
|---|---|---|
| Baseline read | disabled | 7200 seconds |
| Set haptics enabled | matching write acknowledgment: enabled | not written |
| Immediate readback | enabled | not read |
| Read after the requested power cycle | enabled | 7200 seconds |

Thus the haptics setting written by the independent client survives a power cycle
on the tested Dial/firmware. Settings reads and the haptics write are now exercised
independently of Studio. Sleep remained unchanged; its independent persistence
requires changing it before a power cycle. The original haptics value was disabled;
the user subsequently confirmed restoring haptics to disabled.


### Sleep timer persistence across a power cycle

With Studio quit, the user set 3600 seconds using the independent client and
received both a matching write acknowledgment and readback. After the requested
power cycle, a read-only settings query returned 3600 seconds and haptics disabled.
The user then restored 7200 seconds, receiving both acknowledgment and readback.
Thus sleep-timer configuration survives a power cycle on the tested Dial/firmware,
and independent sleep writes are confirmed. Original settings are restored:
haptics disabled and sleep timer 7200 seconds. This experiment verifies retention
and command handling, not actual sleep timing or arbitrary durations.


### Three top buttons held during rotation

With the independent client and normal heartbeats, the user held TopLeft, TopMiddle,
and TopRight while rotating, then released TopMiddle, TopLeft, and TopRight.
All three down transitions and all three up transitions were preserved in the
requested order, with rotation events between them and knob_pressed=false.
The log contained two counterclockwise events and four clockwise events (12 total
control events), although the requested sequence was two detents each direction.
Thus simultaneous tracking of three buttons plus rotation is confirmed, while
exact clockwise event-per-detent behavior for this run remains unresolved: no
synchronized physical detent count or packet timestamps were provided. Do not
infer event duplication or loss from this mismatch alone.


### Client process termination

With Studio quit and the normal events example running, the user rotated the
Dial, confirmed factory volume actions were suppressed, then terminated the
example with Ctrl-C while continuing to rotate. System volume resumed after
approximately three seconds, matching heartbeat expiry rather than immediate
fallback. This tests process termination: the example has no Ctrl-C cleanup
handler, and macOS may retain its system-level connection to the HID device.
An explicit client disconnect/unsubscribe was not established by this experiment
and remains a separate question. A killed client therefore leaves online behavior
active for roughly three seconds on the tested device.


### Explicit local BLE client disconnection

The user ran `events --disconnect-probe` with Studio quit and continued rotating.
After five seconds of online operation, the example sent a fresh normal heartbeat
and awaited `Client::disconnect()` (btleplug peripheral disconnect). The user
reported factory volume control resuming after approximately three seconds.
Thus explicit disconnection of this client's CoreBluetooth/GATT session did not
cause immediate fallback on the tested Mac/Dial; the result matches heartbeat
expiry and Ctrl-C termination. The system HID connection may remain active, so
this is not evidence about termination of the entire physical Bluetooth link.
Explicit unsubscription alone and other platforms remain untested.


### Non-UI sleep duration acceptance

The independent client sent a 60-second sleep duration with Studio quit. The Dial
returned `Reply::SleepTimer { seconds: 60, write_response: true }` and a subsequent
read returned `seconds: 60` with write_response=false. This confirms acceptance
and readback of at least one duration outside Studio's hour-based options.
It does not establish the full accepted range, power-cycle retention of this
non-UI value, or actual sleep timing. At this point the configured timer is
60 seconds; restoration to the user's original 7200 seconds is pending.


### Idle observation with a 60-second timer

After acceptance/readback of 60 seconds, the user stopped normal heartbeats and
ran the passive notification client with USB unplugged, as requested. The user
reported no observable change after four minutes; the Dial remained connected.
No disconnect was observed despite the duration exceeding the configured value.
This does not establish whether the device stayed fully awake, entered a connected
low-power state, ignored/clamped the duration, or had idle timing affected by the
system/client connection. Actual timer semantics remain unresolved. A timestamped
HCI capture around idle and a subsequent physical input is needed to distinguish
connection behavior and wake response.


### HCI capture of idle with a 60-second timer

Capture: d100h-idle-sleep.pklg, with Studio and the independent client stopped
and USB unplugged, per the requested experiment. The Dial vendor/HID traffic is
on connection handle 0x0052 (identified by its previously observed attributes and
decryptable vendor messages; connection establishment predates the trace).
The first press begins at frame 148, trace-relative 489027.177 seconds; the second
at frame 9653, 489124.453 seconds: separation **97.276 seconds**.
Both presses produce the same HID sequence on attribute 0x0032:
0800000000000000 -> 0800060000000000 -> 0000060000000000 -> 0000000000000000.
Their vendor notifications are 8b104101 (down) and 8b104100 (up), an offline
code 0x41, identified as LowerLeft by the later standalone-controls capture.
Both include battery 100% with voltage raw 4123 and no charging/status flags.
No host ATT writes to the Dial, disconnection, reconnection, or LE connection
parameter update occurs within the captured interval. The same connection handle
is used before/after idle. No externally visible sleep transition is established.
Internal low-power behavior cannot be inferred from absence of HCI events, and
physical press times were not independently recorded, so wake latency cannot be
measured. The accepted 60-second setting's actual sleep semantics remain unresolved;
do not equate acceptance/readback with a guaranteed 60-second link disconnection.


### All seven physical buttons held simultaneously

With Studio quit and normal heartbeats, the user pressed and held TopLeft,
TopMiddle, TopRight, UpperLeft, UpperRight, LowerLeft, and LowerRight, then
released in reverse order. All seven down and final up events were observed.
During setup, UpperRight additionally reported an up and another down before
LowerLeft/LowerRight were pressed; those transitions were preserved separately.
Immediately after LowerRight down, the event-derived held set contained all
seven physical buttons. All seven final releases were received in reverse order.
Thus full seven-button simultaneous tracking is confirmed on the tested firmware;
the extra setup transition does not invalidate the seven-button held state.
This does not test the knob pressed or rotation concurrently with all seven.


### Single-step held rotation and release-click suppression

The user ran three separated actions: plain knob press/release, one held-rotation
step in one direction and release, then one held-rotation step in the other
direction and release. The output contained one KnobClick down/up pair for the
plain click, then one held counterclockwise and one held clockwise rotation event
(the direction order in the output differs from the requested order). No further
knob click pair appeared. Thus one reported held-rotation step is sufficient to
suppress the knob click on release in either tested direction. Sub-step physical
movement without any reported rotation remains untested. No event timestamps were
provided; the experiment relies on the user's separated physical actions.


### Release-click suppression resets on subsequent presses

After one held counterclockwise rotation event and physical release, the user
pressed and released the knob twice without rotating. The log contained the one
held rotation followed by two complete KnobClick down/up pairs. Together with the
single-step suppression experiment, this confirms that suppression does not carry
over to later knob presses: subsequent plain presses generate normal release
clicks. The precise internal firmware state is inferred from behavior, not read
from a dedicated state field.


### Independent identity-query verification

With Studio quit, the user ran `events --read-identity`; the independent client
sent each identity request once while maintaining normal heartbeats. Replies
matched the original Studio capture:

| Field | Decoded value |
|---|---|
| Firmware | 1.3.8.1 |
| Custom code | 400037 (three hex bytes 40 00 37) |
| Device Bluetooth address | ca:61:f0:83:e3:fb |
| Flash ID | 4250315a353838020025c6435603e278 |
| Serial fragment 0 | A0D26I003U |
| Serial fragment 1 | 3711117 |
| Reassembled serial | A0D26I003U3711117 |

Fragment index 1 was delivered twice with identical content after the one serial
query. A consumer assembling the serial should deduplicate identical fragments
by index rather than append every notification. This establishes that duplicates
can occur, not their cause. General fragment completion rules remain unverified;
the library continues exposing raw indexed fragments. Query transmission and
typed firmware/address/flash-ID/serial parsing are now verified independently
on this device, not merely against Studio's captured exchanges.


### Slow versus fast rotation capture

Capture: d100h-fast-rotation.pklg, independent client with normal heartbeats.
All vendor ATT values on the Dial channel (connection 0x0052, handles 0x004d/0x0050)
were 20 bytes. Every rotation notification decoded to the existing format
8b 10 ID 01 followed by 16 zero bytes; no count/delta fields or new IDs appeared.

| Phase | Event ID | Events | Minimum inter-event gap |
|---|---|---:|---:|
| Slow free clockwise | 0a | 5 | 225 ms |
| Slow free counterclockwise | 09 | 6 | 195 ms |
| Fast free clockwise | 0a | 13 | 15 ms |
| Fast free counterclockwise | 09 | 17 | 14 ms |
| Fast knob-held clockwise | 0c | 12 | 14 ms |
| Fast knob-held counterclockwise | 0b | 15 | 15 ms |

The sample contains 68 separate rotation notifications, 17 heartbeat replies,
and one battery-status vendor notification. No knob-click pair appeared after held
rotation. Thus the tested fast input uses separate existing event packets rather
than an explicit batched count. Host receipt spacing is not a physical detent-time
measurement. Exact physical detent counts were not recorded, so this experiment
does not establish losslessness, a maximum supported rate, or behavior at speeds
beyond those exercised. No firmware acceleration field was observed.


### Standalone controls: vendor events and HID reports

Capture: `d100h-standalone-controls.pklg`, with Studio quit and the independent
client in passive mode. The user exercised the top buttons from left to right,
then UpperLeft, LowerLeft, UpperRight, LowerRight, a knob click, clockwise
rotation, and counterclockwise rotation. On the Dial's connection `0x0052`,
vendor notifications use ATT handle `0x004d`; HID reports use handles `0x0032`
and `0x003d`. All four observed vendor down/up pairs decrypt to
`8b 10 ID STATE` followed by zeroes, where STATE is `01` for down and `00`
for up:

| Physical side button, by exercise order | Standalone vendor ID | HID handle and nonzero report during action |
|---|---:|---|
| UpperLeft | `42` | `0x0032`: `08 00 19 00 00 00 00 00` |
| LowerLeft | `41` | `0x0032`: `08 00 06 00 00 00 00 00` |
| UpperRight | `44` | `0x0032`: `0a 00 1d 00 00 00 00 00` |
| LowerRight | `43` | `0x0032`: `08 00 1d 00 00 00 00 00` |

The top three buttons produced HID reports on `0x003d` with respective
nonzero values `b6 00`, `cd 00`, and `b5 00`. The knob click also produced
`cd 00` on `0x003d`. Clockwise and counterclockwise rotations produced
`ea 00` and `e9 00` respectively on `0x003d`. Each was followed by a
zero release report. These six actions produced no vendor event on `0x004d`
in this capture. The shared `cd 00` for TopMiddle and the knob click shows
that this standalone HID binding does not preserve their physical identity.

The four `0x0032` keyboard reports were interleaved with vendor down/up
notifications for the same actions. These standalone IDs are distinct from
the online IDs `04` through `08`; a consumer must not decode `41` through
`44` using the online button mapping. The ID-to-button assignments above
follow the user's stated action order and packet chronology. The capture
does not establish whether customized standalone presets change either the
vendor IDs or HID report values. Full decrypted and HID traffic is in
`standalone-controls-decoded.tsv`.


### Notification unsubscription while heartbeats continue

Capture: `d100h-unsubscribe.pklg`, paired with the independent client's
`--unsubscribe-probe` log, with Studio stopped. The client enabled notifications
by writing `01 00` to descriptor handle `0x004e` at 4.254 s. At 9.287 s it wrote
`00 00` to the same descriptor, received the ATT write response at 9.322 s, and
reported successful unsubscription. A single heartbeat stream continued about
once per second through 14.283 s.

The user continuously rotated counterclockwise across the transition. Online
rotation notifications (`8b 10 09 01`) remain visible in the air capture through
11.781 s, about 2.494 s after the descriptor-disable request. The application log
ends its decoded rotation output at the successful unsubscribe, as expected.
No nonzero HID keyboard or consumer-control report appears after unsubscription;
only zero HID reports occur at 12.351 s. Thus notification subscription is not
required to maintain online mode or suppress standalone HID actions while valid
heartbeats continue.

The continued on-air vendor notifications after the acknowledged CCCD disable
are an observed behavior of this firmware/CoreBluetooth path. They may reflect
device-side delay or packets already in the transport path; the capture does not
distinguish those causes. This result concerns online-mode state only: an
application must remain subscribed if it needs to receive vendor events.
