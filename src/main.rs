use std::time::Duration;
mod client;
mod packet;

#[tokio::main]
async fn main() {
    let broker_address = "127.0.0.1";
    let broker_port = 1883;
    let username = None;
    let password = None;

    let mqtt_client_options = client::ClientOptions::new(broker_address.to_string(), broker_port)
        .with_credentials(
            username.unwrap_or("".to_string()),
            password.unwrap_or("".to_string()),
        )
        .with_keep_alive(60);

    let mqtt_client = client::new(mqtt_client_options).await;

    loop {
        mqtt_client
            .publish("a/b".to_string(), "msg".to_string())
            .expect("Failed to send message to client thread.");

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
