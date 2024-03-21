use std::thread;
use std::time::Duration;
mod client;

#[tokio::main]
async fn main() {
    let broker_address = "127.0.0.1:1883";

    let _ = client::client::new(broker_address);

    loop {
        thread::sleep(Duration::from_secs(5));
    }
}
