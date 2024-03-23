use core::panic;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::io::Interest;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task;
use tokio::time;

pub struct Client {
    broker_address: String,

    // Payloads -> PUBLISH
    publish_channel_sender: mpsc::UnboundedSender<String>,
    publish_channel_receiver: Option<mpsc::UnboundedReceiver<String>>,

    // TCP packets -> TCP stream
    raw_tcp_channel_sender: mpsc::UnboundedSender<Vec<u8>>,
    raw_tcp_channel_receiver: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
}

pub async fn new(broker_address: &str) -> Client {
    let broker_address = broker_address.to_string();

    let (publish_channel_sender, publish_channel_receiver) = mpsc::unbounded_channel();
    let (raw_tcp_channel_sender, raw_tcp_channel_receiver) = mpsc::unbounded_channel();

    let mut client = Client {
        broker_address,
        publish_channel_sender,
        publish_channel_receiver: Some(publish_channel_receiver),
        raw_tcp_channel_sender,
        raw_tcp_channel_receiver: Some(raw_tcp_channel_receiver),
    };

    let cloned_client = client.clone(); // client without receivers, to be used outside

    task::spawn(async move {
        client.even_loop().await;
    });

    cloned_client
}

impl Client {
    fn clone(&self) -> Client {
        Client {
            broker_address: self.broker_address.clone(),
            publish_channel_sender: self.publish_channel_sender.clone(),
            raw_tcp_channel_sender: self.raw_tcp_channel_sender.clone(),

            // we only use the receivers to feed the event loop
            // so we don't need to clone them
            publish_channel_receiver: None,
            raw_tcp_channel_receiver: None,
        }
    }

    async fn even_loop(&mut self) {
        let mut publish_channel_receiver = self.publish_channel_receiver.take().unwrap();

        loop {
            // Connect to the MQTT broker
            match TcpStream::connect(self.broker_address.clone()).await {
                Ok(mut stream) => {
                    println!("Connected to MQTT broker successfully.");

                    self.send_connect_packet()
                        .expect("Failed to send CONNECT packet to client thread.");
                    println!("CONNECT message sent successfully.");

                    let mut ping_interval = time::interval(time::Duration::from_secs(10));

                    loop {
                        tokio::select! {
                            _ = ping_interval.tick() => {
                                self.send_pingreq_packet().expect("Failed to send PINGREQ packet to client thread.");
                                println!("PINGREQ message sent successfully.");
                            }
                            Some(msg) = publish_channel_receiver.recv() => {
                                self.send_publish_packet(msg).expect("Failed to send PUBLISH packet to client thread.");
                                println!("PUBLISH message sent successfully.");
                            }
                            Some(packet) = self.raw_tcp_channel_receiver.as_mut().unwrap().recv() => {
                                if let Err(e) = stream.write(&packet).await {
                                    eprintln!("Failed to send raw TCP packet: {}", e);
                                } else {
                                    println!("Raw TCP packet sent successfully.");
                                }
                            }
                            _ = stream.ready(Interest::READABLE) => {
                                let mut response = [0; 128];
                                match stream.try_read(&mut response) {
                                    Ok(n) => {
                                        if n == 0 {
                                            // n == 0 means the other side has closed the connection
                                            println!("Broker closed the connection.");
                                            break;
                                        }

                                        // Parse the broker response
                                        crate::client::packet::parse_incoming_packet(&response[0..n]);
                                    }
                                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
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
    }

    pub fn publish(&self, payload: String) -> Result<(), mpsc::error::SendError<String>> {
        self.publish_channel_sender.send(payload)
    }

    fn send_connect_packet(&self) -> Result<(), mpsc::error::SendError<Vec<u8>>> {
        let connect_packet = crate::client::packet::craft_connect_packet();
        self.raw_tcp_channel_sender.send(connect_packet)
    }

    fn send_pingreq_packet(&self) -> Result<(), mpsc::error::SendError<Vec<u8>>> {
        let pingreq_packet = crate::client::packet::craft_pingreq_packet();
        self.raw_tcp_channel_sender.send(pingreq_packet)
    }

    fn send_publish_packet(&self, payload: String) -> Result<(), mpsc::error::SendError<Vec<u8>>> {
        let publish_packet = crate::client::packet::craft_publish_packet(payload);
        self.raw_tcp_channel_sender.send(publish_packet)
    }
}
