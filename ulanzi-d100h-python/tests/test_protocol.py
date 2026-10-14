import unittest
from datetime import timedelta

from ulanzi_d100h import Command, Packet, PACKET_LEN


def packet(prefix):
    return Packet(bytes(prefix).ljust(PACKET_LEN, b"\0"))


class ProtocolTests(unittest.TestCase):
    def test_wire_roundtrip_and_tail(self):
        original = bytes(range(20))
        wire = Packet(original).to_wire()
        self.assertIsInstance(wire, bytes)
        self.assertNotEqual(wire[:16], original[:16])
        self.assertEqual(wire[16:], original[16:])
        self.assertEqual(Packet.from_wire(wire).plaintext, original)

    def test_invalid_lengths(self):
        for size in (0, 19, 21):
            for constructor in (Packet, Packet.from_wire):
                with self.assertRaises(ValueError):
                    constructor(bytes(size))

    def test_commands(self):
        cases = [
            (Command.heartbeat(), [6, 1, 0x23, 0, 1]),
            (Command.read_haptics(), [1, 6, 0x40, 1]),
            (Command.set_haptics(True), [1, 6, 0x40, 4, 255]),
            (Command.set_haptics(False), [1, 6, 0x40, 4, 0]),
            (Command.read_sleep_timer(), [1, 1, 0x42, 1]),
            (Command.set_sleep_timer(3600), [1, 1, 0x42, 2, 0x10, 0x0e]),
            (Command.read_serial(), [1, 1, 0x0b, 1]),
            (Command.read_firmware_version(), [1, 4, 4, 1]),
            (Command.read_device_address(), [1, 1, 0xfa, 1]),
            (Command.read_flash_id(), [1, 4, 0x0b, 1]),
            (Command.observed_setup(), [1, 1, 0xfa, 1]),
        ]
        for command, prefix in cases:
            self.assertEqual(command.packet().plaintext, packet(prefix).plaintext)
        self.assertEqual(Command.raw(packet([99])).packet().plaintext, packet([99]).plaintext)
        for value in (-1, 65536):
            with self.assertRaises(OverflowError):
                Command.set_sleep_timer(value)
        with self.assertRaises(TypeError):
            Command.set_sleep_timer(1.5)

    def test_sleep_duration(self):
        for seconds in (0, 3600, 65535):
            self.assertEqual(Command.sleep_timer(timedelta(seconds=seconds)).packet().plaintext,
                             Command.set_sleep_timer(seconds).packet().plaintext)
        for duration in (timedelta(seconds=-1), timedelta(seconds=65536),
                         timedelta(microseconds=1), timedelta(days=1)):
            with self.subTest(duration=duration), self.assertRaises(ValueError):
                Command.sleep_timer(duration)
        with self.assertRaises(TypeError):
            Command.sleep_timer(3600)

    def test_device_info_replies(self):
        for revision in (0, 1):
            raw_data = bytes([0x12, 0x34, 0x56, 0, 0, 0, 1, 3, 8, 0, 0, 0, 0, 0, revision, 0])
            value = Packet(bytes([1, 4, 4, 0x11]) + raw_data)
            self.assertEqual(Packet.from_wire(value.to_wire()).message(), {
                "type": "firmware_version", "version": "1.3.8.1" if revision else "1.3.8",
                "major": 1, "minor": 3, "patch": 8, "revision": revision or None,
                "custom_code": b"\x12\x34\x56", "raw_data": raw_data,
            })
        self.assertEqual(packet([1, 1, 0xfa, 0x11, 0xca, 0x61, 0xf0, 0x83, 0xe3, 0xfb]).message(),
                         {"type": "device_address", "address": bytes.fromhex("ca61f083e3fb")})
        data = bytes(range(16))
        self.assertEqual(Packet(bytes([1, 4, 0x0b, 0x11]) + data).message(),
                         {"type": "flash_id", "data": data})

    def test_events(self):
        self.assertEqual(packet([0x8b, 0x10, 1, 1]).message(),
                         {"type": "button", "button": 1, "pressed": True})
        self.assertEqual(packet([0x8b, 0x10, 7, 0]).message(),
                         {"type": "knob_click", "pressed": False})
        for event_id, direction, held in [(9, "counterclockwise", False),
                                          (10, "clockwise", False),
                                          (11, "counterclockwise", True),
                                          (12, "clockwise", True)]:
            self.assertEqual(packet([0x8b, 0x10, event_id, 1]).message(),
                             {"type": "rotation", "direction": direction, "knob_pressed": held})

    def test_replies_and_battery(self):
        self.assertEqual(packet([6, 1, 0x23, 0x10, 1]).message(), {"type": "heartbeat"})
        self.assertEqual(packet([1, 6, 0x40, 0x14, 255]).message(),
                         {"type": "haptics", "enabled": True, "write_response": True})
        self.assertEqual(packet([1, 1, 0x42, 0x11, 0x10, 0x0e]).message(),
                         {"type": "sleep_timer", "seconds": 3600, "write_response": False})
        self.assertEqual(packet([1, 1, 0x0b, 0x11, 2, 1, 255, 0]).message(),
                         {"type": "serial_fragment", "index": 1, "text": b"\xff\0"})
        self.assertEqual(packet([0x0b, 0x7b, 0x22, 0x10, 0x1f, 100]).message(),
                         {"type": "battery_status", "voltage_raw": 4130, "battery_percent": 100,
                          "low_battery_status": 3, "power_off": True, "charging": True,
                          "charge_full": True})

    def test_unknown_preserved(self):
        value = packet([99, 42])
        self.assertEqual(value.message(), {"type": "unknown", "plaintext": value.plaintext})


if __name__ == "__main__":
    unittest.main()
