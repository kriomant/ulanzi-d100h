use std::sync::Arc;
use btleplug::{api::{Central as _, CharPropFlags, Manager as _, Peripheral as _, RetrievePeripheralsOptions}, platform::{Manager, Peripheral}};
use pyo3::{exceptions::PyRuntimeError, prelude::*};
use tokio::sync::{Mutex, OnceCell, mpsc, oneshot};
use ulanzi_d100h::{ble::Client as RustClient, Message};
use crate::{Command, message_dict};

fn error(error: impl std::fmt::Display) -> PyErr { PyRuntimeError::new_err(error.to_string()) }
fn closed() -> PyErr { error("client is disconnected") }

// On macOS, every new btleplug Adapter starts a CoreBluetooth thread without a
// shutdown path. Keep adapters for the extension lifetime so reconnect retries
// do not accumulate threads.
static ADAPTERS: OnceCell<Vec<btleplug::platform::Adapter>> = OnceCell::const_new();

async fn adapters() -> PyResult<&'static [btleplug::platform::Adapter]> {
    ADAPTERS
        .get_or_try_init(|| async {
            let manager = Manager::new().await?;
            manager.adapters().await
        })
        .await
        .map(Vec::as_slice)
        .map_err(error)
}

async fn peripherals() -> PyResult<Vec<Peripheral>> {
    let mut found = Vec::new();
    for adapter in adapters().await? {
        found.extend(adapter.retrieve_peripherals(RetrievePeripheralsOptions {
            services: Some(vec![
                "00001971-0000-1000-8000-00805f9b34fb".parse().unwrap(),
                "00002013-0000-1000-8000-00805f9b34fb".parse().unwrap(),
            ]),
            ..Default::default()
        }).await.map_err(error)?);
    }
    Ok(found)
}

/// List already-connected devices exposing a vendor service.
#[pyfunction]
pub fn discover(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    pyo3_async_runtimes::tokio::future_into_py(py, async {
        let mut devices = Vec::new();
        for peripheral in peripherals().await? {
            let name = peripheral.properties().await.map_err(error)?.and_then(|p| p.local_name);
            devices.push((peripheral.id().to_string(), name));
        }
        Ok(devices)
    })
}

enum Request {
    Send(ulanzi_d100h::Command, oneshot::Sender<PyResult<()>>),
    Disconnect(oneshot::Sender<PyResult<()>>),
}

/// Async BLE client. Call disconnect explicitly when finished.
#[pyclass(frozen, module = "ulanzi_d100h")]
pub struct Client {
    requests: mpsc::Sender<Request>,
    messages: Arc<Mutex<mpsc::UnboundedReceiver<PyResult<Message>>>>,
}

#[pymethods]
impl Client {
    /// Connect to an already-paired device; omit device_id only when one device is available.
    #[staticmethod]
    #[pyo3(signature = (device_id=None))]
    fn connect(py: Python<'_>, device_id: Option<String>) -> PyResult<Bound<'_, PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut found: Vec<_> = peripherals().await?.into_iter()
                .filter(|p| device_id.as_ref().is_none_or(|id| *id == p.id().to_string())).collect();
            if found.len() != 1 {
                return Err(error(format!("expected one connected Dial, found {}; use discover() and select a device_id", found.len())));
            }
            let peripheral = found.remove(0);
            peripheral.connect().await.map_err(error)?;
            let attached = async {
                peripheral.discover_services().await.map_err(error)?;
                let mut pairs = Vec::new();
                for service in peripheral.services() {
                    if !matches!(service.uuid.to_string().as_str(),
                        "00001971-0000-1000-8000-00805f9b34fb" | "00002013-0000-1000-8000-00805f9b34fb") {
                        continue;
                    }
                    for write in &service.characteristics {
                        for notify in &service.characteristics {
                            if write.properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE)
                                && notify.properties.contains(CharPropFlags::NOTIFY) && write.uuid != notify.uuid {
                                pairs.push((write.clone(), notify.clone()));
                            }
                        }
                    }
                }
                if pairs.len() != 1 { return Err(error("expected one unambiguous vendor write/notify pair")); }
                let (write, notify) = pairs.remove(0);
                RustClient::attach(peripheral.clone(), write, notify).await.map_err(error)
            }.await;
            let mut client = match attached {
                Ok(client) => client,
                Err(err) => { let _ = peripheral.disconnect().await; return Err(err); }
            };
            let (requests, mut receiver) = mpsc::channel(16);
            let (sender, messages) = mpsc::unbounded_channel();
            pyo3_async_runtimes::tokio::get_runtime().spawn(async move {
                loop {
                    tokio::select! {
                        request = receiver.recv() => match request {
                            Some(Request::Send(command, reply)) => {
                                let _ = reply.send(client.send(command).await.map_err(error));
                            }
                            Some(Request::Disconnect(reply)) => {
                                let _ = reply.send(client.disconnect().await.map_err(error));
                                return;
                            }
                            None => break,
                        },
                        message = client.next_message() => match message {
                            Some(message) => { if sender.send(message.map_err(error)).is_err() { break; } }
                            None => break,
                        }
                    }
                }
                let _ = client.disconnect().await;
            });
            Ok(Self { requests, messages: Arc::new(Mutex::new(messages)) })
        })
    }

    fn send<'py>(&self, py: Python<'py>, command: &Command) -> PyResult<Bound<'py, PyAny>> {
        let requests = self.requests.clone();
        let command = command.0;
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (tx, rx) = oneshot::channel();
            requests.send(Request::Send(command, tx)).await.map_err(|_| closed())?;
            rx.await.map_err(|_| closed())?
        })
    }

    fn next_message<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let messages = self.messages.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let message = messages.lock().await.recv().await.transpose()?;
            Python::attach(|py| message.map(|message| message_dict(py, message).map(Bound::unbind)).transpose())
        })
    }

    fn disconnect<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let requests = self.requests.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let (tx, rx) = oneshot::channel();
            if requests.send(Request::Disconnect(tx)).await.is_err() { return Ok(()); }
            rx.await.map_err(|_| closed())?
        })
    }
}
