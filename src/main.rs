use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

fn main() {
    let broker_address = "127.0.0.1:1883";

    loop {
        // Connect to the MQTT broker
        match TcpStream::connect(broker_address) {
            Ok(mut stream) => {
                // Craft the CONNECT message
                let connect_packet = craft_connect_packet();

                // Send the CONNECT message
                if let Err(e) = stream.write(&connect_packet) {
                    eprintln!("Failed to send CONNECT message: {}", e);
                    return;
                }

                println!("CONNECT message sent successfully.");

                loop {
                    // Read the response from the broker
                    let mut response = [0; 128];
                    match stream.read(&mut response) {
                        Ok(n) => {
                            if n == 0 {
                                // n == 0 means the other side has closed the connection
                                println!("Broker closed the connection.");
                                break;
                            }
                            println!("Broker response: {:?}", &response[0..n]);

                            // Parse the broker response
                            parse_incoming_packet(&response[0..n]);

                            // Craft the PUBLISH message
                            let publish_packet = craft_publish_packet();

                            // Send the PUBLISH message
                            if let Err(e) = stream.write(&publish_packet) {
                                eprintln!("Failed to send PUBLISH message: {}", e);
                                return;
                            } else {
                                println!("PUBLISH message sent successfully.");
                            }

                            thread::sleep(Duration::from_secs(5));

                            // Craft the PINGREQ message
                            let pingreq_packet = craft_pingreq_packet();

                            // Send the PINGREQ message
                            if let Err(e) = stream.write(&pingreq_packet) {
                                eprintln!("Failed to send PINGREQ message: {}", e);
                                return;
                            } else {
                                println!("PINGREQ message sent successfully.");
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to read broker response: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to connect to MQTT broker: {}", e);
                // Retry connection after 5 seconds
                thread::sleep(Duration::from_secs(5));
            }
        }
    }
}

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

fn craft_connect_packet() -> Vec<u8> {
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

fn craft_publish_packet() -> Vec<u8> {
    // Construct the PUBLISH packet according to MQTT protocol specification
    let mut packet = Vec::new();

    /*
    =======================
    Fixed header
    =======================
    */

    packet.push(u8::from_be_bytes([0b0011_0000])); // PUBLISH message type (0011 + DUP + QoS Level + RETAIN
    packet.push(0x00); // Remaining Length (variable header + payload) -- placeholder, will be computed later

    /*
    =======================
    Variable header
    =======================
    */

    packet.push(0x00); // Topic name Length MSB
    packet.push(0x03); // Topic name Length LSB //TODO: compute this dynamically
    packet.extend_from_slice(b"a/b"); // Topic Name

    /*
    =======================
    Payload
    =======================
    */

    packet.extend_from_slice(b"Hello, MQTT :D"); // Payload

    // Compute "Remaining Length" field of the fixed header: length of variable header + payload
    let remaining_length = packet.len() - 2; // Subtract 2 bytes for the fixed header
    packet[1] = remaining_length as u8;

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

fn parse_connack_packet(packet: &[u8]) {
    // Parse the CONNACK packet according to MQTT protocol specification
    let connack_flags = packet[2];
    let connack_return_code = packet[3];

    println!("CONNACK Flags: {:08b}", connack_flags); // all bites to 0, last bit is session present
    println!(
        "CONNACK Return Code: {:?}",
        ConnackReturnCode::from(connack_return_code)
    );
}

fn parse_pingresp_packet(packet: &[u8]) {
    if packet[0] != 0b1101_0000 {
        eprintln!("Invalid PINGRESP packet.");
    }

    // Parse the PINGRESP packet according to MQTT protocol specification
    println!("PINGRESP packet received.");
}

fn parse_incoming_packet(packet: &[u8]) {
    // Parse the incoming packet according to MQTT protocol specification
    let packet_type = packet[0] >> 4;

    match packet_type {
        2 => parse_connack_packet(packet),
        13 => parse_pingresp_packet(packet),
        _ => println!("Unsupported packet type: {}", packet_type),
    }
}

fn craft_pingreq_packet() -> Vec<u8> {
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
