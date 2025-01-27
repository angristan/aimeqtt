use std::{fmt, io};

#[derive(Debug)]
pub enum MqttError {
    ConnectionClosed,
    PacketTooLarge,
    InvalidPacket(String),
    IoError(std::io::Error),
}

impl std::error::Error for MqttError {}

impl fmt::Display for MqttError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MqttError::ConnectionClosed => write!(f, "Broker closed the connection"),
            MqttError::PacketTooLarge => write!(f, "Received packet exceeds maximum size"),
            MqttError::InvalidPacket(msg) => write!(f, "Invalid packet: {}", msg),
            MqttError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl From<std::io::Error> for MqttError {
    fn from(error: std::io::Error) -> Self {
        MqttError::IoError(error)
    }
}

impl From<MqttError> for io::Error {
    fn from(error: MqttError) -> Self {
        match error {
            MqttError::ConnectionClosed => io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "Connection closed by broker",
            ),
            MqttError::InvalidPacket(msg) => io::Error::new(io::ErrorKind::InvalidData, msg),
            MqttError::PacketTooLarge => {
                io::Error::new(io::ErrorKind::InvalidData, "Packet too large")
            }
            MqttError::IoError(error) => error,
        }
    }
}
