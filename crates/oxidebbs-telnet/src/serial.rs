use std::fmt;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::transport::{Transport, TransportError};

#[derive(Debug, Clone)]
pub struct SerialPortConfig {
    pub path: String,
    pub baud_rate: u32,
    pub data_bits: u8,
    pub parity: SerialParity,
    pub stop_bits: u8,
    pub flow_control: SerialFlowControl,
    pub init_strings: Vec<String>,
    pub answer_string: Option<String>,
    pub require_carrier_detect: bool,
    pub drop_dtr_on_hangup: bool,
    pub read_timeout_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialParity {
    None,
    Odd,
    Even,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialFlowControl {
    None,
    RtsCts,
    XonXoff,
}

impl Default for SerialPortConfig {
    fn default() -> Self {
        Self {
            path: String::new(),
            baud_rate: 115_200,
            data_bits: 8,
            parity: SerialParity::None,
            stop_bits: 1,
            flow_control: SerialFlowControl::RtsCts,
            init_strings: Vec::new(),
            answer_string: None,
            require_carrier_detect: false,
            drop_dtr_on_hangup: true,
            read_timeout_ms: 100,
        }
    }
}

impl SerialPortConfig {
    fn to_serialport_parity(&self) -> serialport::Parity {
        match self.parity {
            SerialParity::None => serialport::Parity::None,
            SerialParity::Odd => serialport::Parity::Odd,
            SerialParity::Even => serialport::Parity::Even,
        }
    }

    fn to_serialport_stop_bits(&self) -> serialport::StopBits {
        match self.stop_bits {
            2 => serialport::StopBits::Two,
            _ => serialport::StopBits::One,
        }
    }

    fn to_serialport_flow_control(&self) -> serialport::FlowControl {
        match self.flow_control {
            SerialFlowControl::None => serialport::FlowControl::None,
            SerialFlowControl::RtsCts => serialport::FlowControl::Hardware,
            SerialFlowControl::XonXoff => serialport::FlowControl::Software,
        }
    }

    fn to_serialport_data_bits(&self) -> serialport::DataBits {
        match self.data_bits {
            5 => serialport::DataBits::Five,
            6 => serialport::DataBits::Six,
            7 => serialport::DataBits::Seven,
            _ => serialport::DataBits::Eight,
        }
    }
}

pub struct SerialTransport {
    port: Arc<Mutex<Box<dyn serialport::SerialPort>>>,
    config: SerialPortConfig,
}

impl fmt::Debug for SerialTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SerialTransport")
            .field("path", &self.config.path)
            .field("baud_rate", &self.config.baud_rate)
            .finish_non_exhaustive()
    }
}

impl SerialTransport {
    pub fn open(config: SerialPortConfig) -> Result<Self, SerialOpenError> {
        let mut port = serialport::new(&config.path, config.baud_rate)
            .data_bits(config.to_serialport_data_bits())
            .parity(config.to_serialport_parity())
            .stop_bits(config.to_serialport_stop_bits())
            .flow_control(config.to_serialport_flow_control())
            .timeout(Duration::from_millis(config.read_timeout_ms))
            .open()
            .map_err(|e| SerialOpenError::DeviceOpen {
                path: config.path.clone(),
                reason: e.to_string(),
            })?;

        for init_string in &config.init_strings {
            let bytes = if init_string.ends_with('\r') {
                init_string.as_bytes().to_vec()
            } else {
                let mut v = init_string.as_bytes().to_vec();
                v.push(b'\r');
                v
            };
            port.write_all(&bytes)
                .map_err(|e| SerialOpenError::ModemInit {
                    path: config.path.clone(),
                    reason: e.to_string(),
                })?;
            port.flush().ok();
            std::thread::sleep(Duration::from_millis(100));
        }

        if let Some(answer) = &config.answer_string {
            let bytes = if answer.ends_with('\r') {
                answer.as_bytes().to_vec()
            } else {
                let mut v = answer.as_bytes().to_vec();
                v.push(b'\r');
                v
            };
            port.write_all(&bytes)
                .map_err(|e| SerialOpenError::ModemInit {
                    path: config.path.clone(),
                    reason: e.to_string(),
                })?;
            port.flush().ok();
        }

        info!(
            path = %config.path,
            baud_rate = config.baud_rate,
            "Serial port opened"
        );

        Ok(Self {
            port: Arc::new(Mutex::new(port)),
            config,
        })
    }

    fn read_byte_sync(port: Arc<Mutex<Box<dyn serialport::SerialPort>>>) -> Option<u8> {
        let mut buf = [0u8; 1];
        let mut guard = port.lock().ok()?;
        match guard.read(&mut buf) {
            Ok(1) => Some(buf[0]),
            Ok(_) => None,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => None,
            Err(_) => None,
        }
    }
}

impl Transport for SerialTransport {
    async fn read_byte(&mut self) -> Result<Option<u8>, TransportError> {
        let port = self.port.clone();
        let require_cd = self.config.require_carrier_detect;

        tokio::task::spawn_blocking(move || {
            if require_cd {
                let mut guard = port.lock().map_err(|_| TransportError::Closed)?;
                match guard.read_carrier_detect() {
                    Ok(false) => return Ok(None),
                    Err(_) => {}
                    Ok(true) => {}
                }
                drop(guard);
            }
            Ok(SerialTransport::read_byte_sync(port))
        })
        .await
        .map_err(|_| TransportError::Closed)?
    }

    async fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        let port = self.port.clone();
        let bytes = bytes.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut guard = port.lock().map_err(|_| TransportError::Closed)?;
            guard
                .write_all(&bytes)
                .map_err(|_| TransportError::Closed)?;
            guard.flush().ok();
            Ok(())
        })
        .await
        .map_err(|_| TransportError::Closed)?
    }

    async fn hangup(&mut self) -> Result<(), TransportError> {
        let port = self.port.clone();
        let drop_dtr = self.config.drop_dtr_on_hangup;
        tokio::task::spawn_blocking(move || {
            if drop_dtr
                && let Ok(mut guard) = port.lock()
                && let Err(e) = guard.write_data_terminal_ready(false)
            {
                warn!("Failed to drop DTR on hangup: {}", e);
            }
            Ok(())
        })
        .await
        .map_err(|_| TransportError::Closed)?
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SerialOpenError {
    #[error("failed to open serial device {path}: {reason}")]
    DeviceOpen { path: String, reason: String },
    #[error("modem initialization failed on {path}: {reason}")]
    ModemInit { path: String, reason: String },
    #[error("serial line-state feature not supported on this platform: {feature}")]
    UnsupportedLineState { feature: String },
}

pub struct SerialLoopback {
    rx: mpsc::UnboundedReceiver<u8>,
    tx: mpsc::UnboundedSender<u8>,
}

impl fmt::Debug for SerialLoopback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SerialLoopback").finish_non_exhaustive()
    }
}

impl SerialLoopback {
    pub fn new() -> (Self, SerialHandle) {
        let (client_tx, server_rx) = mpsc::unbounded_channel();
        let (server_tx, client_rx) = mpsc::unbounded_channel();
        (
            Self {
                rx: server_rx,
                tx: server_tx,
            },
            SerialHandle {
                rx: client_rx,
                tx: client_tx,
            },
        )
    }
}

impl Transport for SerialLoopback {
    async fn read_byte(&mut self) -> Result<Option<u8>, TransportError> {
        Ok(self.rx.recv().await)
    }

    async fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        for &byte in bytes {
            self.tx.send(byte).map_err(|_| TransportError::Closed)?;
        }
        Ok(())
    }

    async fn hangup(&mut self) -> Result<(), TransportError> {
        self.tx = mpsc::unbounded_channel().0;
        Ok(())
    }
}

pub struct SerialHandle {
    rx: mpsc::UnboundedReceiver<u8>,
    tx: mpsc::UnboundedSender<u8>,
}

impl SerialHandle {
    pub async fn read_byte(&mut self) -> Option<u8> {
        self.rx.recv().await
    }

    pub fn write_byte(&self, byte: u8) -> Result<(), TransportError> {
        self.tx.send(byte).map_err(|_| TransportError::Closed)
    }

    pub fn write_bytes(&self, bytes: &[u8]) -> Result<(), TransportError> {
        for &byte in bytes {
            self.tx.send(byte).map_err(|_| TransportError::Closed)?;
        }
        Ok(())
    }

    pub fn read_output_bytes(&mut self) -> Vec<u8> {
        let mut bytes = Vec::new();
        while let Ok(byte) = self.rx.try_recv() {
            bytes.push(byte);
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn serial_echo_roundtrip() {
        let (mut server, mut client) = SerialLoopback::new();

        client.write_bytes(b"Hi").expect("write to server");
        let first = server.read_byte().await.expect("read first");
        let second = server.read_byte().await.expect("read second");

        assert_eq!(first, Some(b'H'));
        assert_eq!(second, Some(b'i'));

        server.write_all(b"OK").await.expect("write to client");
        let out = client.read_output_bytes();
        assert_eq!(out, b"OK");
    }

    #[tokio::test]
    async fn serial_hangup_returns_closed() {
        let (mut server, client) = SerialLoopback::new();

        server.hangup().await.expect("hangup");
        let result = server.write_all(b"x").await;
        assert!(matches!(result, Err(TransportError::Closed)));

        drop(client);
        let byte = server.read_byte().await.expect("read after close");
        assert_eq!(byte, None);
    }

    #[test]
    fn serial_port_config_defaults() {
        let config = SerialPortConfig::default();
        assert_eq!(config.baud_rate, 115_200);
        assert_eq!(config.data_bits, 8);
        assert_eq!(config.parity, SerialParity::None);
        assert_eq!(config.stop_bits, 1);
        assert_eq!(config.flow_control, SerialFlowControl::RtsCts);
        assert!(!config.require_carrier_detect);
        assert!(config.drop_dtr_on_hangup);
    }

    #[test]
    fn serial_open_error_display() {
        let err = SerialOpenError::DeviceOpen {
            path: "/dev/ttyUSB0".to_string(),
            reason: "Permission denied".to_string(),
        };
        assert!(err.to_string().contains("/dev/ttyUSB0"));
        assert!(err.to_string().contains("Permission denied"));

        let err = SerialOpenError::UnsupportedLineState {
            feature: "carrier detect".to_string(),
        };
        assert!(err.to_string().contains("carrier detect"));
    }
}
