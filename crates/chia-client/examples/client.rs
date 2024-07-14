use std::time::Duration;

use chia_client::{create_tls_connector, Client, ClientOptions, Event};
use chia_ssl::ChiaCertificate;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    log::info!("Generating certificate");
    let cert = ChiaCertificate::generate()?;
    let tls_connector = create_tls_connector(cert.cert_pem.as_bytes(), cert.key_pem.as_bytes())?;

    log::info!("Creating client");
    let (client, mut receiver) = Client::with_options(
        tls_connector,
        ClientOptions {
            target_peers: 20,
            ..Default::default()
        },
    );

    log::info!("Connecting to DNS introducers");
    client.find_peers().await;

    let client_clone = client.clone();

    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(10)).await;
            let count = client_clone.len().await;
            log::info!("Currently connected to {} peers", count);
            client.find_peers().await;
        }
    });

    while let Some(event) = receiver.recv().await {
        match event {
            Event::Message(peer_id, message) => {
                log::info!(
                    "Received message from peer {}: {:?}",
                    peer_id,
                    message.msg_type
                );
            }
            Event::ConnectionClosed(peer_id) => {
                log::info!("Peer {} disconnected", peer_id);
            }
        }
    }

    Ok(())
}
