use core::panic;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

use tokio::task;

pub fn new(broker_address: &str) {
    let broker_address = broker_address.to_string();
    task::spawn(async move {
        loop {
            // Connect to the MQTT broker
            match TcpStream::connect(broker_address.clone()) {
                Ok(mut stream) => {
                    // Craft the CONNECT message
                    let connect_packet = crate::client::packet::craft_connect_packet();

                    // Send the CONNECT message
                    if let Err(e) = stream.write(&connect_packet) {
                        eprintln!("Failed to send CONNECT message: {}", e);
                        panic!("Failed to send CONNECT message: {}", e);
                    }

                    println!("CONNECT message sent successfully.");

                    loop {
                        println!("Reading broker response...");
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
                                crate::client::packet::parse_incoming_packet(&response[0..n]);

                                // Craft the PUBLISH message
                                let publish_packet =
                                    crate::client::packet::craft_publish_packet("yo".to_string());

                                // Send the PUBLISH message
                                if let Err(e) = stream.write(&publish_packet) {
                                    eprintln!("Failed to send PUBLISH message: {}", e);
                                    panic!("Failed to send PUBLISH message: {}", e);
                                } else {
                                    println!("PUBLISH message sent successfully.");
                                }

                                thread::sleep(Duration::from_secs(5));

                                // Craft the PINGREQ message
                                let pingreq_packet = crate::client::packet::craft_pingreq_packet();

                                // Send the PINGREQ message
                                if let Err(e) = stream.write(&pingreq_packet) {
                                    eprintln!("Failed to send PINGREQ message: {}", e);
                                    panic!("Failed to send PINGREQ message: {}", e);
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
    });

    return ();
}
