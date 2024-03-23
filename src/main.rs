use std::time::Duration;
mod client;
mod packet;

#[tokio::main]
async fn main() {
    let broker_address = "127.0.0.1:1883";

    let mqtt_client = client::new(broker_address).await;

    loop {
        mqtt_client
            .publish("msg".to_string())
            .expect("Failed to send message to client thread.");

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
