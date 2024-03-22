use core::panic;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::io::Interest;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task;
use tokio::time;

pub async fn new(broker_address: &str) -> mpsc::UnboundedSender<String> {
    let broker_address = broker_address.to_string();

    let (tx, mut rx) = mpsc::unbounded_channel();

    task::spawn(async move {
        loop {
            // Connect to the MQTT broker
            match TcpStream::connect(broker_address.clone()).await {
                Ok(mut stream) => {
                    // let (mut stream, mut stream) = tokio::io::split(stream);

                    // Craft the CONNECT message
                    let connect_packet = crate::client::packet::craft_connect_packet();

                    // Send the CONNECT message
                    if let Err(e) = stream.write(&connect_packet).await {
                        eprintln!("Failed to send CONNECT message: {}", e);
                        panic!("Failed to send CONNECT message: {}", e);
                    }

                    println!("CONNECT message sent successfully.");

                    let mut ping_interval = time::interval(time::Duration::from_secs(10));

                    loop {
                        // println!("tokio::select");
                        tokio::select! {
                            _ = ping_interval.tick() => {
                                // println!("ping_interval.tick()");
                                // Craft the PINGREQ message
                                let pingreq_packet = crate::client::packet::craft_pingreq_packet();

                                // Send the PINGREQ message
                                if let Err(e) = stream.write(&pingreq_packet).await {
                                    eprintln!("Failed to send PINGREQ message: {}", e);
                                    panic!("Failed to send PINGREQ message: {}", e);
                                } else {
                                    println!("PINGREQ message sent successfully.");
                                }
                            }
                            Some(msg) = rx.recv() => {
                                // println!("rx.recv_async()");
                                    // println!("Received message from main thread: {:?}", msg);

                                    // Craft the PUBLISH message
                                    let publish_packet =
                                    crate::client::packet::craft_publish_packet(msg);

                                    // Send the PUBLISH message
                                    if let Err(e) = stream.write(&publish_packet).await {
                                        eprintln!("Failed to send PUBLISH message: {}", e);
                                        panic!("Failed to send PUBLISH message: {}", e);
                                    } else {
                                        println!("PUBLISH message sent successfully.");
                                    }
                            }
                            _ = stream.ready(Interest::READABLE) => {
                                            // println!("The stream is ready to read data.");
                                            let mut response = [0; 128];
                                            match stream.try_read(&mut response) {
                                                Ok(n) => {
                                                if n == 0 {
                                                    // n == 0 means the other side has closed the connection
                                                    println!("Broker closed the connection.");
                                                    break;
                                                }
                                                println!("Broker response: {:?}", &response[0..n]);

                                                // Parse the broker response
                                                crate::client::packet::parse_incoming_packet(&response[0..n]);


                                                // // Craft the PINGREQ message
                                                // let pingreq_packet = crate::client::packet::craft_pingreq_packet();

                                                // // Send the PINGREQ message
                                                // if let Err(e) = stream.write(&pingreq_packet).await {
                                                //     eprintln!("Failed to send PINGREQ message: {}", e);
                                                //     panic!("Failed to send PINGREQ message: {}", e);
                                                // } else {
                                                //     println!("PINGREQ message sent successfully.");
                                                // }
                                            }
                                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                                // println!("!!!The stream is not ready to read data yet.");
                                                // There's no data to read at the moment
                                                continue;
                                            }
                                            Err(e) => {
                                                eprintln!("Failed to read broker response: {}", e);
                                                break;
                                            }
                                        }




                            }
                        }
                    }
                }

                Err(e) => {
                    eprintln!("Failed to connect to MQTT broker: {}", e);
                    // Retry connection after 5 seconds
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });

    tx
}
