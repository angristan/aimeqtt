// Ref: https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html
// https://public.dhe.ibm.com/software/dw/webservices/ws-mqtt/mqtt-v3r1.html

use std::time::Duration;

use tracing::{event, Level};

#[derive(Clone)]
pub enum PacketType {
    CONNECT = 1,
    CONNACK = 2,
    PUBLISH = 3,
    SUBSCRIBE = 8,
    SUBACK = 9,
    PINGREQ = 12,
    PINGRESP = 13,
}

impl From<u8> for PacketType {
    fn from(code: u8) -> Self {
        match code {
            1 => PacketType::CONNECT,
            2 => PacketType::CONNACK,
            3 => PacketType::PUBLISH,
            8 => PacketType::SUBSCRIBE,
            9 => PacketType::SUBACK,
            12 => PacketType::PINGREQ,
            13 => PacketType::PINGRESP,
            _ => panic!("Invalid packet type: {}", code),
        }
    }
}

struct Packet {
    packet_type: PacketType,
    username: Option<String>,
    password: Option<String>,
    keep_alive: Option<Duration>,
    client_id: String,
    packet_id: Option<u8>,
    topic: Option<String>,
    topic_filter: Option<String>,
    message: Option<String>,
}

impl Packet {
    fn new(packet_type: PacketType) -> Packet {
        Packet {
            packet_type,
            username: None,
            password: None,
            keep_alive: None,
            client_id: "".to_string(),
            packet_id: None,
            topic: None,
            topic_filter: None,
            message: None,
        }
    }

    fn with_username(mut self, username: String) -> Packet {
        self.username = Some(username);
        self
    }

    fn with_password(mut self, password: String) -> Packet {
        self.password = Some(password);
        self
    }

    fn with_keep_alive(mut self, keep_alive: Duration) -> Packet {
        self.keep_alive = Some(keep_alive);
        self
    }

    fn with_client_id(mut self, client_id: String) -> Packet {
        self.client_id = client_id;
        self
    }

    fn with_packet_id(mut self, packet_id: u8) -> Packet {
        self.packet_id = Some(packet_id);
        self
    }

    fn with_topic(mut self, topic: String) -> Packet {
        self.topic = Some(topic);
        self
    }

    fn with_topic_filter(mut self, topic_filter: String) -> Packet {
        self.topic_filter = Some(topic_filter);
        self
    }

    fn with_message(mut self, message: String) -> Packet {
        self.message = Some(message);
        self
    }
}

struct ConnectFlags {
    username_flag: u8,
    password_flag: u8,
    will_retain: u8,
    will_qos: u8,
    will_flag: u8,
    clean_session: u8,
}

impl ConnectFlags {
    fn to_byte(&self) -> u8 {
        self.username_flag << 7
            | self.password_flag << 6
            | self.will_retain << 5
            | self.will_qos << 3 // QoS level is 2 bits (0, 1, 2)
            | self.will_flag << 2
            | self.clean_session << 1
            | 0 // reserved
    }
}

struct RawPacket {
    fixed_header: Vec<u8>,
    variable_header: Vec<u8>,
    payload: Vec<u8>,
}

impl RawPacket {
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.fixed_header);
        bytes.extend_from_slice(&self.variable_header);
        bytes.extend_from_slice(&self.payload);
        bytes
    }
}

const PROTOCOL_NAME: &str = "MQTT";
const PROTOCOL_LEVEL: u8 = 4; // MQTT 3.1.1

impl Packet {
    fn to_raw_packet(&self) -> RawPacket {
        let mut packet = RawPacket {
            fixed_header: Vec::new(),
            variable_header: Vec::new(),
            payload: Vec::new(),
        };

        packet
            .fixed_header
            .push((self.packet_type.clone() as u8) << 4); // Packet Type

        match self.packet_type {
            PacketType::CONNECT => {
                packet.variable_header.push(0x00); // Protocol Name Length MSB
                packet.variable_header.push(PROTOCOL_NAME.len() as u8); // Protocol Name Length LSB
                packet
                    .variable_header
                    .extend_from_slice(PROTOCOL_NAME.as_bytes()); // Protocol Name

                packet.variable_header.push(PROTOCOL_LEVEL); // Protocol Level

                let connect_flags = ConnectFlags {
                    username_flag: if self.username.is_some() { 1 } else { 0 },
                    password_flag: if self.password.is_some() { 1 } else { 0 },
                    will_retain: 0,
                    will_qos: 0,
                    will_flag: 0,
                    clean_session: 1,
                };
                packet.variable_header.push(connect_flags.to_byte());

                // Keep Alive
                packet.variable_header.push(0x00); // Keep Alive MSB
                packet
                    .variable_header
                    .push(u8::from_be(self.keep_alive.unwrap().as_secs() as u8)); // Keep Alive LSB

                // Client id
                packet.payload.push(0x00); // Client ID Length MSB
                packet.payload.push(self.client_id.len() as u8); // Client ID Length LSB
                packet.payload.extend_from_slice(self.client_id.as_bytes()); // Client ID

                // Auth
                if self.username.is_some() {
                    packet.payload.push(0x00); // Username Length MSB
                    packet
                        .payload
                        .push(self.username.as_ref().unwrap().len() as u8); // Username Length LSB
                    packet
                        .payload
                        .extend_from_slice(self.username.as_ref().unwrap().as_bytes());
                }
                if self.password.is_some() {
                    packet.payload.push(0x00); // Password Length MSB
                    packet
                        .payload
                        .push(self.password.as_ref().unwrap().len() as u8); // Password Length LSB
                    packet
                        .payload
                        .extend_from_slice(self.password.as_ref().unwrap().as_bytes());
                }
            }
            PacketType::PUBLISH => {
                packet.variable_header.push(0x00); // Topic name Length MSB
                packet
                    .variable_header
                    .push(self.topic.as_ref().unwrap().len() as u8); // Topic name Length LSB
                packet
                    .variable_header
                    .extend_from_slice(self.topic.as_ref().unwrap().as_bytes()); // Topic Name

                // Packet Identifier - optional for QoS 0
                // packet.variable_header.push(0x00); // Packet Identifier MSB
                // packet.variable_header.push(self.packet_id.unwrap() as u8); // Packet Identifier LSB

                packet
                    .payload
                    .extend_from_slice(self.message.as_ref().unwrap().as_bytes());
            }
            PacketType::SUBSCRIBE => {
                packet.fixed_header[0] |= 0b0000_0010; // Set QoS = 1

                packet.variable_header.push(0x00); // Packet Identifier MSB
                packet.variable_header.push(0x01); // Packet Identifier LSB

                packet.variable_header.push(0x00); // Topic Filter Length MSB
                packet
                    .variable_header
                    .push(self.topic_filter.as_ref().unwrap().len() as u8); // Topic Filter Length LSB
                packet
                    .variable_header
                    .extend_from_slice(self.topic_filter.as_ref().unwrap().as_bytes()); // Topic Filter

                packet.variable_header.push(0x00); // Requested QoS
            }
            _ => {}
        }

        match packet.variable_header.len() + packet.payload.len() {
            0 => packet.fixed_header.push(0x00),
            _ => {
                // Compute "Remaining Length" field of the fixed header
                let mut remaining_length = packet.variable_header.len() + packet.payload.len();

                // Encode the "Remaining Length" field as per MQTT protocol specification:
                /*
                   The Remaining Length is encoded using a variable length encoding scheme which uses a single byte for values up to 127.
                   Larger values are handled as follows. The least significant seven bits of each byte encode the data,
                   and the most significant bit is used to indicate that there are following bytes in the representation.
                   Thus each byte encodes 128 values and a "continuation bit". The maximum number of bytes in the Remaining Length field is four.

                   This allows applications to send Control Packets of size up to 268,435,455 (256 MB)
                */
                let mut encoded_bytes: Vec<u8> = vec![];
                while remaining_length > 0 {
                    let mut byte = (remaining_length % 128) as u8;
                    remaining_length /= 128;
                    if remaining_length > 0 {
                        byte |= u8::from_be(128);
                    }
                    encoded_bytes.push(byte);
                }

                // Add the encoded bytes (1 up to 4) representing the "Remaining Length" field, starting from the second byte of the fixed header
                packet
                    .fixed_header
                    .splice(1..1, encoded_bytes.iter().cloned());
            }
        }

        packet
    }
}

pub fn craft_connect_packet(username: Option<String>, password: Option<String>) -> Vec<u8> {
    Packet::new(PacketType::CONNECT)
        .with_client_id("rust".to_string())
        .with_keep_alive(Duration::from_secs(10))
        .with_username(username.unwrap_or("".to_string()))
        .with_password(password.unwrap_or("".to_string()))
        .to_raw_packet()
        .to_bytes()
}

pub fn craft_publish_packet(topic: String, payload: String) -> Vec<u8> {
    Packet::new(PacketType::PUBLISH)
        .with_client_id("rust".to_string())
        .with_topic(topic)
        .with_message(payload)
        .to_raw_packet()
        .to_bytes()
}

pub fn craft_pingreq_packet() -> Vec<u8> {
    Packet::new(PacketType::PINGREQ)
        .with_client_id("rust".to_string())
        .to_raw_packet()
        .to_bytes()
}

pub fn craft_subscribe_packet(topic_filter: String) -> Vec<u8> {
    Packet::new(PacketType::SUBSCRIBE)
        .with_client_id("rust".to_string())
        .with_topic_filter(topic_filter)
        .to_raw_packet()
        .to_bytes()
}

#[derive(Debug)]
enum ConnackReturnCode {
    ConnectionAccepted = 0,
    ConnectionRefusedUnacceptableProtocolVersion = 1,
    ConnectionRefusedIdentifierRejected = 2,
    ConnectionRefusedServerUnavailable = 3,
    ConnectionRefusedBadUsernameOrPassword = 4,
    ConnectionRefusedNotAuthorized = 5,
}

impl From<u8> for ConnackReturnCode {
    fn from(code: u8) -> Self {
        match code {
            0 => ConnackReturnCode::ConnectionAccepted,
            1 => ConnackReturnCode::ConnectionRefusedUnacceptableProtocolVersion,
            2 => ConnackReturnCode::ConnectionRefusedIdentifierRejected,
            3 => ConnackReturnCode::ConnectionRefusedServerUnavailable,
            4 => ConnackReturnCode::ConnectionRefusedBadUsernameOrPassword,
            5 => ConnackReturnCode::ConnectionRefusedNotAuthorized,
            _ => panic!("Invalid CONNACK return code: {}", code),
        }
    }
}

pub fn parse_connack_packet(packet: &[u8]) {
    // Parse the CONNACK packet according to MQTT protocol specification
    let connack_flags = packet[2];
    let connack_return_code = packet[3];

    event!(Level::DEBUG, "CONNACK Flags: {:08b}", connack_flags); // all bites to 0, last bit is session present
    event!(
        Level::DEBUG,
        "CONNACK Return Code: {:?}",
        ConnackReturnCode::from(connack_return_code)
    );
}

pub fn parse_pingresp_packet(packet: &[u8]) {
    if packet[0] != 0b1101_0000 {
        event!(Level::ERROR, "Invalid PINGRESP packet.");
    }

    // Parse the PINGRESP packet according to MQTT protocol specification
    event!(Level::DEBUG, "PINGRESP packet received.");
}

pub fn parse_suback_packet(packet: &[u8]) {
    // Parse the SUBACK packet according to MQTT protocol specification
    let packet_id = (packet[2] as u16) << 8 | packet[3] as u16;
    let return_code = packet[4];

    event!(Level::DEBUG, "SUBACK Packet ID: {}", packet_id);
    event!(Level::DEBUG, "SUBACK Return Code: {}", return_code);
}

pub fn parse_publish_packet(packet: &[u8]) -> (String, String) {
    // Parse the PUBLISH packet according to MQTT protocol specification
    let topic_length = (packet[2] as u16) << 8 | packet[3] as u16;
    let topic = std::str::from_utf8(&packet[4..(4 + topic_length as usize)]).unwrap();

    let message = std::str::from_utf8(&packet[(4 + topic_length as usize)..]).unwrap();

    event!(Level::DEBUG, "PUBLISH Topic: {}", topic);
    event!(Level::DEBUG, "PUBLISH Message: {}", message);

    (topic.to_string(), message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_craft_connect_packet() {
        let packet = craft_connect_packet(None, None);
        assert_eq!(
            packet,
            vec![16, 16, 0, 4, 77, 81, 84, 84, 4, 2, 0, 10, 0, 4, 114, 117, 115, 116]
        );
    }

    #[test]
    fn test_craft_publish_packet_auth() {
        let packet = craft_connect_packet(Some("user".to_string()), Some("pass".to_string()));
        assert_eq!(
            packet,
            vec![
                16, 28, 0, 4, 77, 81, 84, 84, 4, 194, 0, 10, 0, 4, 114, 117, 115, 116, 0, 4, 117,
                115, 101, 114, 0, 4, 112, 97, 115, 115
            ]
        );
    }

    #[test]
    fn test_craft_publish_packet() {
        let packet = craft_publish_packet("a/b".to_string(), "Hello, MQTT!".to_string());
        assert_eq!(
            packet,
            vec![48, 17, 0, 3, 97, 47, 98, 72, 101, 108, 108, 111, 44, 32, 77, 81, 84, 84, 33]
        );
    }

    #[test]
    fn test_craft_pingreq_packet() {
        let packet = craft_pingreq_packet();
        assert_eq!(packet, vec![192, 0]);
    }
}
