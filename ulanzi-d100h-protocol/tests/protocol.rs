use std::time::Duration;
use ulanzi_d100h_protocol::{Command, Event, Message, Packet};

#[test]
fn malformed_packets_and_unknown_events_are_preserved() {
    assert!(Packet::from_wire(&[0; 19]).is_err());
    assert!(Packet::from_wire(&[0; 21]).is_err());
    let mut p = [0; 20];
    p[..4].copy_from_slice(&[0x8b, 0x10, 0x20, 1]);
    assert_eq!(Packet(p).message(), Message::Unknown(Packet(p)));
    p[..4].copy_from_slice(&[0x8b, 0x10, 1, 2]);
    assert_eq!(Packet(p).message(), Message::Unknown(Packet(p)));
    p[19] = 0x42;
    assert_eq!(Packet::from_wire(&Packet(p).to_wire()).unwrap().0[19], 0x42);
}

#[test]
fn builds_ui_verified_settings() {
    assert_eq!(&Command::SetHaptics(true).packet().0[..5], &[1, 6, 0x40, 4, 0xff]);
    assert_eq!(&Command::SetHaptics(false).packet().0[..5], &[1, 6, 0x40, 4, 0]);
    for seconds in [0u16, 3600, 7200, 10800, 14400] {
        let p = Command::sleep_timer(Duration::from_secs(u64::from(seconds))).unwrap().packet();
        assert_eq!(&p.0[..4], &[1, 1, 0x42, 2]);
        assert_eq!(&p.0[4..6], &seconds.to_le_bytes());
    }
    assert!(Command::sleep_timer(Duration::from_millis(1500)).is_err());
    assert!(Command::sleep_timer(Duration::from_secs(65536)).is_err());
}

#[test]
fn all_controls_and_held_rotation_parse() {
    for id in 1..=12 {
        let mut p = [0; 20];
        p[..4].copy_from_slice(&[0x8b, 0x10, id, 1]);
        let event = Packet(p).message();
        assert!(matches!(event, Message::Event(_)));
        if id >= 9 {
            assert!(matches!(event, Message::Event(Event::Rotation { knob_pressed, .. }) if knob_pressed == (id >= 11)));
        }
    }
}

#[test]
fn decodes_observed_battery_notification() {
    let packet = Packet([11, 123, 34, 16, 0, 100, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(Packet::from_wire(&packet.to_wire()).unwrap().message(),
        Message::BatteryStatus(ulanzi_d100h_protocol::BatteryStatus {
            voltage_raw: 4130,
            battery_percent: 100,
            low_battery_status: 0,
            power_off: false,
            charging: false,
            charge_full: false,
        }));
}

#[test]
fn battery_flags_and_unrecognized_fields() {
    let mut p = [0; 20];
    p[..6].copy_from_slice(&[0x0b, 0x7b, 0x78, 0x10, 0, 100]);
    for flags in 0..32 {
        p[4] = flags;
        let Message::BatteryStatus(status) = Packet(p).message() else { panic!("battery report not recognized") };
        assert_eq!(status.voltage_raw, 4216);
        assert_eq!(status.low_battery_status, flags & 3);
        assert_eq!(status.power_off, flags & 4 != 0);
        assert_eq!(status.charging, flags & 8 != 0);
        assert_eq!(status.charge_full, flags & 16 != 0);
    }
    p[4] = 0x20;
    assert_eq!(Packet(p).message(), Message::Unknown(Packet(p)));
    p[4] = 0;
    p[6] = 1;
    assert_eq!(Packet(p).message(), Message::Unknown(Packet(p)));
}
