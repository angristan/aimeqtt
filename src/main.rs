use std::io::{Read, Write};
use std::net::TcpStream;

fn main() {
    let server_address = "127.0.0.1:1883";

    // Connect to the MQTT broker
    match TcpStream::connect(server_address) {
        Ok(mut stream) => {
            // Craft the CONNECT message
            let connect_packet = craft_connect_packet();

            // Send the CONNECT message
            if let Err(e) = stream.write(&connect_packet) {
                eprintln!("Failed to send CONNECT message: {}", e);
                return;
            }

            println!("CONNECT message sent successfully.");

            // Read the response from the server
            let mut response = [0; 128];
            match stream.read(&mut response) {
                Ok(_) => {
                    println!("Server response: {:?}", response);

                    // Craft the PUBLISH message
                    let publish_packet = craft_publish_packet();

                    // Send the PUBLISH message
                    if let Err(e) = stream.write(&publish_packet) {
                        eprintln!("Failed to send PUBLISH message: {}", e);
                        return;
                    }
                }
                Err(e) => {
                    eprintln!("Failed to read server response: {}", e);
                    return;
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to connect to MQTT broker: {}", e);
        }
    }
}

// Ref: https://docs.oasis-open.org/mqtt/mqtt/v3.1.1/os/mqtt-v3.1.1-os.html

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

    // Connect Flags
    // Bit	7	            6	            5	            4 3	        2           1	            0
    //      username flag	password flag   Will retain     Will QoS    Will flag	Clean session   reserved
    //      x	            x	            x	            xx	        x	        x	           	0
    packet.push(u8::from_be(0b0000_0010)); // (Clean Session)

    // Keep Alive
    packet.push(0x00); // Keep Alive MSB
    packet.push(u8::from_be(60)); // Keep Alive LSB (60 seconds)

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
