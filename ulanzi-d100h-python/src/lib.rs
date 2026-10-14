mod ble;

use std::time::Duration;
use pyo3::{exceptions::PyValueError, prelude::*, types::{PyBytes, PyDict}};
use ulanzi_d100h_protocol::{Command as RustCommand, Direction, Event, Message, Packet as RustPacket, Reply};

/// A plaintext 20-byte vendor packet.
#[pyclass(frozen, module = "ulanzi_d100h")]
struct Packet(RustPacket);

#[pymethods]
impl Packet {
    #[new]
    fn new(plaintext: &[u8]) -> PyResult<Self> {
        let bytes = plaintext.try_into().map_err(|_| PyValueError::new_err("expected 20-byte plaintext packet"))?;
        Ok(Self(RustPacket(bytes)))
    }

    #[staticmethod]
    fn from_wire(wire: &[u8]) -> PyResult<Self> {
        RustPacket::from_wire(wire).map(Self).map_err(|error| PyValueError::new_err(error.to_string()))
    }

    #[getter]
    fn plaintext<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.0)
    }

    fn to_wire<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.0.to_wire())
    }

    /// Decode into a dictionary with a `type` discriminator.
    fn message<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        message_dict(py, self.0.message())
    }
}

/// A vendor command, constructed with the named factory methods.
#[pyclass(frozen, module = "ulanzi_d100h")]
struct Command(RustCommand);

#[pymethods]
impl Command {
    #[staticmethod]
    fn heartbeat() -> Self { Self(RustCommand::Heartbeat) }
    #[staticmethod]
    fn read_haptics() -> Self { Self(RustCommand::ReadHaptics) }
    #[staticmethod]
    fn set_haptics(enabled: bool) -> Self { Self(RustCommand::SetHaptics(enabled)) }
    #[staticmethod]
    fn read_sleep_timer() -> Self { Self(RustCommand::ReadSleepTimer) }
    #[staticmethod]
    fn set_sleep_timer(seconds: u16) -> Self { Self(RustCommand::SetSleepTimer(seconds)) }
    #[staticmethod]
    fn sleep_timer(duration: Duration) -> PyResult<Self> {
        RustCommand::sleep_timer(duration).map(Self).map_err(|error| PyValueError::new_err(error.to_string()))
    }
    #[staticmethod]
    fn read_serial() -> Self { Self(RustCommand::ReadSerial) }
    #[staticmethod]
    fn read_firmware_version() -> Self { Self(RustCommand::ReadFirmwareVersion) }
    #[staticmethod]
    fn read_device_address() -> Self { Self(RustCommand::ReadDeviceAddress) }
    #[staticmethod]
    fn read_flash_id() -> Self { Self(RustCommand::ReadFlashId) }
    #[staticmethod]
    fn observed_setup() -> Self { Self(RustCommand::ObservedSetup) }
    #[staticmethod]
    fn raw(packet: &Packet) -> Self { Self(RustCommand::Raw(packet.0)) }
    fn packet(&self) -> Packet { Packet(self.0.packet()) }
}

fn message_dict(py: Python<'_>, message: Message) -> PyResult<Bound<'_, PyDict>> {
    let result = PyDict::new(py);
    let kind = match message {
        Message::Event(Event::Button { button, pressed }) => {
            result.set_item("button", button as u8)?;
            result.set_item("pressed", pressed)?;
            "button"
        }
        Message::Event(Event::KnobClick { pressed }) => {
            result.set_item("pressed", pressed)?;
            "knob_click"
        }
        Message::Event(Event::Rotation { direction, knob_pressed }) => {
            result.set_item("direction", match direction {
                Direction::Clockwise => "clockwise", Direction::Counterclockwise => "counterclockwise",
            })?;
            result.set_item("knob_pressed", knob_pressed)?;
            "rotation"
        }
        Message::Reply(Reply::Heartbeat) => "heartbeat",
        Message::Reply(Reply::FirmwareVersion { version, custom_code, raw_data }) => {
            result.set_item("version", version.to_string())?;
            result.set_item("major", version.major)?;
            result.set_item("minor", version.minor)?;
            result.set_item("patch", version.patch)?;
            result.set_item("revision", version.revision)?;
            result.set_item("custom_code", PyBytes::new(py, &custom_code))?;
            result.set_item("raw_data", PyBytes::new(py, &raw_data))?;
            "firmware_version"
        }
        Message::Reply(Reply::DeviceAddress(address)) => {
            result.set_item("address", PyBytes::new(py, &address))?;
            "device_address"
        }
        Message::Reply(Reply::FlashId(data)) => {
            result.set_item("data", PyBytes::new(py, &data))?;
            "flash_id"
        }
        Message::Reply(Reply::Haptics { enabled, write_response }) => {
            result.set_item("enabled", enabled)?;
            result.set_item("write_response", write_response)?;
            "haptics"
        }
        Message::Reply(Reply::SleepTimer { seconds, write_response }) => {
            result.set_item("seconds", seconds)?;
            result.set_item("write_response", write_response)?;
            "sleep_timer"
        }
        Message::Reply(Reply::SerialFragment { index, text }) => {
            result.set_item("index", index)?;
            result.set_item("text", PyBytes::new(py, &text))?;
            "serial_fragment"
        }
        Message::BatteryStatus(status) => {
            result.set_item("voltage_raw", status.voltage_raw)?;
            result.set_item("battery_percent", status.battery_percent)?;
            result.set_item("low_battery_status", status.low_battery_status)?;
            result.set_item("power_off", status.power_off)?;
            result.set_item("charging", status.charging)?;
            result.set_item("charge_full", status.charge_full)?;
            "battery_status"
        }
        Message::Unknown(packet) => {
            result.set_item("plaintext", PyBytes::new(py, &packet.0))?;
            "unknown"
        }
    };
    result.set_item("type", kind)?;
    Ok(result)
}

#[pymodule]
fn ulanzi_d100h(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<ble::Client>()?;
    module.add_function(wrap_pyfunction!(ble::discover, module)?)?;
    module.add_class::<Packet>()?;
    module.add_class::<Command>()?;
    module.add("PACKET_LEN", ulanzi_d100h_protocol::PACKET_LEN)?;
    Ok(())
}
