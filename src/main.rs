use std::time::Duration;

mod client;
mod packet;

#[tokio::main]
async fn main() {
    let broker_host = "127.0.0.1";
    let broker_port = 1883;

    let mqtt_client_options = client::ClientOptions::new()
        .with_broker_host(broker_host.to_string())
        .with_broker_port(broker_port)
        .with_keep_alive(60)
        .with_callback_handler(callback_handler);

    let mut mqtt_client = client::new(mqtt_client_options).await;

    mqtt_client
        .subscribe("a/b".to_string())
        .expect("Failed to subscribe to topic.");

    loop {
        mqtt_client
            .publish("a/b".to_string(), "msg".to_string())
            .await
            .expect("Failed to send message to client");

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn callback_handler(payload: String) {
    println!("Received message: {payload}");
}
