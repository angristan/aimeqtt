use std::time::Duration;

use tokio::sync::oneshot;
mod client;
mod packet;

#[tokio::main]
async fn main() {
    let broker_host = "127.0.0.1";
    let broker_port = 1883;

    let mqtt_client_options = client::ClientOptions::new(broker_host.to_string(), broker_port)
        .with_keep_alive(60)
        .with_callback_handler(callback_handler);

    let mut mqtt_client = client::new(mqtt_client_options).await;

    mqtt_client
        .subscribe("a/b".to_string())
        .expect("Failed to subscribe to topic.");

    loop {
        let (resp_tx, resp_rx) = oneshot::channel();

        mqtt_client
            .publish("a/b".to_string(), "msg".to_string(), resp_tx)
            .expect("Failed to send message to client");

        let res = resp_rx
            .await
            .expect("Failed to receive response from client thread.");

        res.expect("Failed to publish message.");

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

fn callback_handler(payload: String) {
    println!("Received message: {}", payload);
}
