use core::panic;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::io::Interest;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task;
use tokio::time;
use tracing::event;
use tracing::Level;

use crate::error::MqttError;

pub struct PublishRequest {
    pub topic: String,
    pub payload: String,
    pub responder: Responder<()>,
}

#[derive(Debug)]
pub enum ClientError {
    InternalError,
}

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

type Responder<T> = oneshot::Sender<Result<T, mpsc::error::SendError<Vec<u8>>>>;

#[derive(Default)]
pub struct ClientOptions<H, P> {
    broker_host: H,
    broker_port: P,
    username: Option<String>,
    password: Option<String>,
    keep_alive: Option<u16>,
    callback_handler: Option<fn(String)>,
}

#[derive(Default, Clone)]
pub struct MissingBroker;
#[derive(Default, Clone)]
pub struct Broker(String);

#[derive(Default, Clone)]
pub struct MissingPort;
#[derive(Default, Clone)]
pub struct Port(u16);

impl ClientOptions<MissingBroker, MissingPort> {
    pub fn new() -> Self {
        ClientOptions::default()
    }
}

impl<B, P> ClientOptions<B, P> {
    pub fn with_broker_host(self, broker_host: String) -> ClientOptions<Broker, P> {
        ClientOptions {
            broker_host: Broker(broker_host),
            broker_port: self.broker_port,
            username: self.username,
            password: self.password,
            keep_alive: self.keep_alive,
            callback_handler: self.callback_handler,
        }
    }

    pub fn with_broker_port(self, broker_port: u16) -> ClientOptions<B, Port> {
        ClientOptions {
            broker_host: self.broker_host,
            broker_port: Port(broker_port),
            username: self.username,
            password: self.password,
            keep_alive: self.keep_alive,
            callback_handler: self.callback_handler,
        }
    }

    pub fn with_credentials(mut self, username: String, password: String) -> ClientOptions<B, P> {
        self.username = Some(username);
        self.password = Some(password);
        self
    }

    pub fn with_keep_alive(mut self, keep_alive: u16) -> ClientOptions<B, P> {
        self.keep_alive = Some(keep_alive);
        self
    }

    pub fn with_callback_handler(mut self, callback_handler: fn(String)) -> ClientOptions<B, P> {
        self.callback_handler = Some(callback_handler);
        self
    }
}

pub async fn new(options: ClientOptions<Broker, Port>) -> Client {
    let broker_address = format!("{}:{}", options.broker_host.0, options.broker_port.0);

    let (publish_channel_sender, publish_channel_receiver) = mpsc::unbounded_channel();
    let (raw_tcp_channel_sender, raw_tcp_channel_receiver) = mpsc::unbounded_channel();

    let mut client = Client {
        broker_address,
        keep_alive: options.keep_alive.unwrap_or(60),
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
        client.event_loop().await;
    });

    cloned_client
}

impl Clone for Client {
    fn clone(&self) -> Self {
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
}

impl Client {
    async fn event_loop(&mut self) {
        let mut publish_channel_receiver = self.publish_channel_receiver.take().unwrap();

        loop {
            match self.connect_to_broker().await {
                Ok(mut stream) => {
                    if let Err(e) = self
                        .handle_connection(&mut stream, &mut publish_channel_receiver)
                        .await
                    {
                        event!(Level::ERROR, "Connection error: {}", e);
                    }
                    // Retry connection after error
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                Err(e) => {
                    event!(Level::ERROR, "Failed to connect to MQTT broker: {}", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn connect_to_broker(&self) -> std::io::Result<TcpStream> {
        let mut stream = TcpStream::connect(self.broker_address.clone()).await?;
        event!(Level::DEBUG, "Connected to MQTT broker successfully.");

        let connect_packet =
            crate::packet::craft_connect_packet(self.username.clone(), self.password.clone());

        stream.write_all(&connect_packet).await?;
        event!(Level::DEBUG, "CONNECT message sent successfully.");

        Ok(stream)
    }

    async fn handle_connection(
        &mut self,
        stream: &mut TcpStream,
        publish_channel_receiver: &mut mpsc::UnboundedReceiver<PublishRequest>,
    ) -> std::io::Result<()> {
        let mut ping_interval = time::interval(Duration::from_secs(self.keep_alive as u64) / 2);

        loop {
            tokio::select! {
                _ = ping_interval.tick() => {
                    self.handle_ping().await?;
                }
                Some(publish_req) = publish_channel_receiver.recv() => {
                    self.handle_publish(publish_req).await?;
                }
                Some(packet) = self.raw_tcp_channel_receiver.as_mut().unwrap().recv() => {
                    self.handle_raw_packet(stream, packet).await?;
                }
                _ = stream.ready(Interest::READABLE) => {
                    match self.handle_incoming_packet(stream).await {
                        Ok(()) => continue,
                        Err(MqttError::ConnectionClosed) => {
                            event!(Level::INFO, "Broker closed connection");
                            return Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, "Broker closed connection"));
                        }
                        Err(e) => {
                            event!(Level::ERROR, "Failed to handle incoming packet: {}", e);
                            return Err(e.into());
                        }
                    }
                }
            }
        }
    }

    async fn handle_ping(&self) -> std::io::Result<()> {
        if let Err(e) = self.send_pingreq_packet() {
            event!(Level::ERROR, "Failed to send PINGREQ message: {}", e);
        } else {
            event!(Level::DEBUG, "PINGREQ message sent successfully.");
        }
        Ok(())
    }

    async fn handle_publish(&self, publish_req: PublishRequest) -> std::io::Result<()> {
        let result = self.send_publish_packet(publish_req.topic, publish_req.payload);
        match result {
            Ok(_) => {
                event!(Level::DEBUG, "PUBLISH message sent successfully.");
                let _ = publish_req.responder.send(Ok(()));
            }
            Err(e) => {
                event!(Level::ERROR, "Failed to send PUBLISH message: {}", e);
                let _ = publish_req.responder.send(Err(e));
            }
        }
        Ok(())
    }

    async fn handle_incoming_packet(&self, stream: &mut TcpStream) -> Result<(), MqttError> {
        let mut response = [0; 128];

        match stream.try_read(&mut response) {
            Ok(0) => Err(MqttError::ConnectionClosed),
            Ok(n) => {
                if n >= response.len() {
                    return Err(MqttError::PacketTooLarge);
                }
                let packet = &response[0..n];
                self.process_packet(packet);
                Ok(())
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    fn process_packet(&self, packet: &[u8]) {
        let packet_type = packet[0] >> 4;
        match crate::packet::PacketType::from(packet_type) {
            crate::packet::PacketType::CONNACK => crate::packet::parse_connack_packet(packet),
            crate::packet::PacketType::PUBLISH => {
                let (_, payload) = crate::packet::parse_publish_packet(packet);
                if let Some(callback_handler) = self.callback_handler {
                    tokio::spawn(async move {
                        callback_handler(payload);
                    });
                }
            }
            _ => event!(Level::DEBUG, "Unsupported packet type: {}", packet_type),
        }
    }

    async fn handle_raw_packet(
        &self,
        stream: &mut TcpStream,
        packet: Vec<u8>,
    ) -> std::io::Result<()> {
        match stream.write_all(&packet).await {
            Ok(_) => {
                event!(Level::DEBUG, "Raw packet sent successfully");
                Ok(())
            }
            Err(e) => {
                event!(Level::ERROR, "Failed to send raw packet: {}", e);
                Err(e)
            }
        }
    }

    pub async fn publish(&self, topic: String, payload: String) -> Result<(), ClientError> {
        let (resp_tx, resp_rx) = oneshot::channel();

        self.publish_channel_sender
            .send(PublishRequest {
                topic,
                payload,
                responder: resp_tx,
            })
            .map_err(|e| {
                event!(
                    Level::ERROR,
                    "Failed to send PUBLISH request to event loop: {}",
                    e
                );
                ClientError::InternalError
            })?;

        resp_rx
            .await
            .map_err(|e| {
                event!(
                    Level::ERROR,
                    "Failed to receive response from event loop: {}",
                    e
                );
                ClientError::InternalError
            })?
            .map_err(|e| {
                event!(Level::ERROR, "Failed to publish message: {}", e);
                ClientError::InternalError
            })?;

        Ok(())
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
