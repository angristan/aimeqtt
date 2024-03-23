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
    keep_alive: u16,

    username: Option<String>,
    password: Option<String>,

    // Payloads -> PUBLISH
    publish_channel_sender: mpsc::UnboundedSender<PublishRequest>,
    publish_channel_receiver: Option<mpsc::UnboundedReceiver<PublishRequest>>,

    // TCP packets -> TCP stream
    raw_tcp_channel_sender: mpsc::UnboundedSender<Vec<u8>>,
    raw_tcp_channel_receiver: Option<mpsc::UnboundedReceiver<Vec<u8>>>,

    callback_handler: Option<fn(String)>,
}

pub struct PublishRequest {
    pub topic: String,
    pub payload: String,
}

pub struct ClientOptions {
    pub broker_host: String,
    pub broker_port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub keep_alive: u16,
    pub callback_handler: Option<fn(String)>,
}

impl ClientOptions {
    pub fn new(broker_host: String, broker_port: u16) -> ClientOptions {
        ClientOptions {
            broker_host,
            broker_port,
            username: None,
            password: None,
            keep_alive: 60,
            callback_handler: None,
        }
    }

    pub fn with_credentials(mut self, username: String, password: String) -> ClientOptions {
        self.username = Some(username);
        self.password = Some(password);
        self
    }

    pub fn with_keep_alive(mut self, keep_alive: u16) -> ClientOptions {
        self.keep_alive = keep_alive;
        self
    }

    pub fn with_callback_handler(mut self, callback_handler: fn(String)) -> ClientOptions {
        self.callback_handler = Some(callback_handler);
        self
    }
}

pub async fn new(options: ClientOptions) -> Client {
    let broker_address = format!("{}:{}", options.broker_host, options.broker_port);

    let (publish_channel_sender, publish_channel_receiver) = mpsc::unbounded_channel();
    let (raw_tcp_channel_sender, raw_tcp_channel_receiver) = mpsc::unbounded_channel();

    let mut client = Client {
        broker_address,
        keep_alive: options.keep_alive,
        username: options.username,
        password: options.password,
        publish_channel_sender,
        publish_channel_receiver: Some(publish_channel_receiver),
        raw_tcp_channel_sender,
        raw_tcp_channel_receiver: Some(raw_tcp_channel_receiver),
        callback_handler: options.callback_handler,
    };

    let cloned_client = client.clone(); // client without receivers, to be used outside

    task::spawn(async move {
        client.even_loop().await;
    });

    cloned_client
}

impl Client {
    pub fn clone(&self) -> Client {
        Client {
            broker_address: self.broker_address.clone(),
            keep_alive: self.keep_alive,
            username: self.username.clone(),
            password: self.password.clone(),
            publish_channel_sender: self.publish_channel_sender.clone(),
            raw_tcp_channel_sender: self.raw_tcp_channel_sender.clone(),
            callback_handler: self.callback_handler,

            // we only use the receivers to feed the event loop
            // so we don't need to clone them
            publish_channel_receiver: None,
            raw_tcp_channel_receiver: None,
        }
    }

    async fn even_loop(&mut self) {
        let mut publish_channel_receiver = self.publish_channel_receiver.take().unwrap();

        // Broker connection loop
        loop {
            match TcpStream::connect(self.broker_address.clone()).await {
                Err(e) => {
                    eprintln!("Failed to connect to MQTT broker: {}", e);

                    // Retry connection after 5 seconds
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                Ok(mut stream) => {
                    println!("Connected to MQTT broker successfully.");

                    let connect_packet = crate::packet::craft_connect_packet(
                        self.username.clone(),
                        self.password.clone(),
                    );

                    match stream.write(&connect_packet).await {
                        Ok(_) => println!("CONNECT message sent successfully."),
                        Err(e) => eprintln!("Failed to send CONNECT message: {}", e),
                    }

                    // Send PINGREQ at least once every keep_alive seconds
                    // so the broker knows the client is still alive
                    let mut ping_interval =
                        time::interval(Duration::from_secs(self.keep_alive as u64) / 2);

                    // Event loop
                    loop {
                        tokio::select! {
                            _ = ping_interval.tick() => {
                                match self.send_pingreq_packet() {
                                    Ok(_) => println!("PINGREQ message sent successfully."),
                                    Err(e) => eprintln!("Failed to send PINGREQ message: {}", e),
                                }
                            }
                            Some(msg) = publish_channel_receiver.recv() => {
                                match self.send_publish_packet(msg.topic, msg.payload) {
                                    Ok(_) => println!("PUBLISH message sent successfully."),
                                    Err(e) => eprintln!("Failed to send PUBLISH message: {}", e),
                                }
                            }
                            Some(packet) = self.raw_tcp_channel_receiver.as_mut().unwrap().recv() => {
                                match stream.write(&packet).await {
                                    Ok(_) => println!("Raw TCP packet sent successfully."),
                                    Err(e) => eprintln!("Failed to send raw TCP packet: {}", e),
                                }
                            }
                            _ = stream.ready(Interest::READABLE) => {
                                let mut response = [0; 128];
                                match stream.try_read(&mut response) {
                                    Ok(n) => {
                                        if n == 0 {
                                            println!("Broker closed the connection.");
                                            break;
                                        }

                                        let packet = &response[0..n];
                                        // Parse the incoming packet according to MQTT protocol specification
                                        let packet_type = packet[0] >> 4;

                                        match crate::packet::PacketType::from(packet_type) {
                                            crate::packet::PacketType::CONNACK => crate::packet::parse_connack_packet(packet),
                                            crate::packet::PacketType::PUBLISH => {
                                                let (_, payload) = crate::packet::parse_publish_packet(packet);
                                                if let Some(callback_handler) = self.callback_handler {
                                                    // TODO: this is blocking the event loop, we should run this in a separate task
                                                    callback_handler(payload);
                                                }
                                            }
                                            crate::packet::PacketType::SUBACK => crate::packet::parse_suback_packet(packet),
                                            crate::packet::PacketType::PINGRESP => crate::packet::parse_pingresp_packet(packet),
                                            _ => println!("Unsupported packet type: {}", packet_type),
                                        }
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
            }
        }
    }

    pub fn publish(
        &self,
        topic: String,
        payload: String,
    ) -> Result<(), mpsc::error::SendError<PublishRequest>> {
        self.publish_channel_sender
            .send(PublishRequest { topic, payload })
    }

    pub fn subscribe(
        &mut self,
        topic_filter: String,
    ) -> Result<(), mpsc::error::SendError<Vec<u8>>> {
        let subscribe_packet = crate::packet::craft_subscribe_packet(topic_filter);
        self.raw_tcp_channel_sender.send(subscribe_packet)
    }

    fn send_pingreq_packet(&self) -> Result<(), mpsc::error::SendError<Vec<u8>>> {
        let pingreq_packet = crate::packet::craft_pingreq_packet();
        self.raw_tcp_channel_sender.send(pingreq_packet)
    }

    fn send_publish_packet(
        &self,
        topic: String,
        payload: String,
    ) -> Result<(), mpsc::error::SendError<Vec<u8>>> {
        let publish_packet = crate::packet::craft_publish_packet(topic, payload);
        self.raw_tcp_channel_sender.send(publish_packet)
    }
}
