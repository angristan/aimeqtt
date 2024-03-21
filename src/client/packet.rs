// Ref: https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html

// Connect Flags
// Bit	7	            6	            5	            4 3	        2           1	            0
//      username flag	password flag   Will retain     Will QoS    Will flag	Clean session   reserved
//      x	            x	            x	            xx	        x	        x	           	0
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

pub fn craft_connect_packet() -> Vec<u8> {
    // Construct the CONNECT packet according to MQTT protocol specification
    let mut packet = Vec::new();

    /*
    =======================
    Fixed header
    =======================
    */

    // Fixed header (CONNECT)
    packet.push(u8::from_be_bytes([0b0001_0000])); // CONNECT message type
    packet.push(0x00); // Remaining Length (variable header + payload) -- placeholder, will be computed later

    /*
    =======================
    Variable header
    =======================
    */

    packet.push(0x00); // Protocol Name Length MSB
    packet.push(0x04); // Protocol Name Length LSB
    packet.extend_from_slice(b"MQTT"); // Protocol Name

    packet.push(0x04); // Protocol Level (4 for MQTT 3.1.1)

    let connect_flags = ConnectFlags {
        username_flag: 0,
        password_flag: 0,
        will_retain: 0,
        will_qos: 0,
        will_flag: 0,
        clean_session: 1,
    };
    packet.push(connect_flags.to_byte());

    // Keep Alive
    packet.push(0x00); // Keep Alive MSB
    packet.push(u8::from_be(10)); // Keep Alive LSB (10 seconds)

    /*
    =======================
    Payload
    =======================
    */

    // Client id
    packet.push(0x00); // Client ID Length MSB
    packet.push(0x04); // Client ID Length LSB //TODO: compute this dynamically
    packet.extend_from_slice(b"rust"); // Client IDs

    // Compute "Remaining Length" field of the fixed header: length of variable header + payload
    let remaining_length = packet.len() - 2; // Subtract 2 bytes for the fixed header
    packet[1] = remaining_length as u8;

    packet
}

pub fn craft_publish_packet(payload: String) -> Vec<u8> {
    // Construct the PUBLISH packet according to MQTT protocol specification
    let mut packet = Vec::new();

    /*
    =======================
    Fixed header
    =======================
    */

    packet.push(u8::from_be_bytes([0b0011_0000])); // PUBLISH message type (0011 + DUP + QoS Level + RETAIN

    // Here should be the 'Remaining Length' field

    /*
    =======================
    Variable header
    =======================
    */

    packet.push(0x00); // Topic name Length MSB
    packet.push(0x03); // Topic name Length LSB //TODO: compute this dynamically
    packet.extend_from_slice(b"a/b"); // Topic Name

    // Packet Identifier - optional for QoS 0
    packet.push(0x00); // Packet Identifier MSB
    packet.push(0x00); // Packet Identifier LSB

    /*
    =======================
    Payload
    =======================
    */

    packet.extend_from_slice(payload.as_bytes()); // Payload

    // Compute "Remaining Length" field of the fixed header
    let mut remaining_length = packet.len() - 1; // variable header + payload = total length - fixed header

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
    packet.splice(1..1, encoded_bytes.iter().cloned());

    packet
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

    println!("CONNACK Flags: {:08b}", connack_flags); // all bites to 0, last bit is session present
    println!(
        "CONNACK Return Code: {:?}",
        ConnackReturnCode::from(connack_return_code)
    );
}

pub fn parse_pingresp_packet(packet: &[u8]) {
    if packet[0] != 0b1101_0000 {
        eprintln!("Invalid PINGRESP packet.");
    }

    // Parse the PINGRESP packet according to MQTT protocol specification
    println!("PINGRESP packet received.");
}

pub fn parse_incoming_packet(packet: &[u8]) {
    // Parse the incoming packet according to MQTT protocol specification
    let packet_type = packet[0] >> 4;

    match packet_type {
        2 => parse_connack_packet(packet),
        13 => parse_pingresp_packet(packet),
        _ => println!("Unsupported packet type: {}", packet_type),
    }
}

pub fn craft_pingreq_packet() -> Vec<u8> {
    // Construct the PINGREQ packet according to MQTT protocol specification
    let mut packet = Vec::new();

    /*
    =======================
    Fixed header
    =======================
    */

    packet.push(u8::from_be_bytes([0b1100_0000])); // PINGREQ message type
    packet.push(0x00); // Remaining Length

    packet
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_craft_connect_packet() {
        let packet = craft_connect_packet();
        assert_eq!(
            packet,
            vec![16, 16, 0, 4, 77, 81, 84, 84, 4, 2, 0, 60, 0, 4, 114, 117, 115, 116]
        );
    }
}
