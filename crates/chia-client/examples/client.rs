use std::time::Duration;

use chia_client::{create_tls_connector, Client, ClientOptions, Network};
use chia_protocol::NodeType;
use chia_ssl::ChiaCertificate;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    log::info!("Generating certificate");
    let cert = ChiaCertificate::generate()?;
    let tls_connector = create_tls_connector(cert.cert_pem.as_bytes(), cert.key_pem.as_bytes())?;

    log::info!("Creating client");
    let (client, mut receiver) = Client::new(
        tls_connector,
        ClientOptions {
            network: Network::mainnet(),
            target_peers: 20,
            connection_concurrency: 10,
            node_type: NodeType::Wallet,
            capabilities: vec![
                (1, "1".to_string()),
                (2, "1".to_string()),
                (3, "1".to_string()),
            ],
            protocol_version: "0.0.34".parse()?,
            software_version: "0.0.0".to_string(),
            connection_timeout: Duration::from_secs(3),
            handshake_timeout: Duration::from_secs(2),
            request_peers_timeout: Duration::from_secs(3),
        },
    );

    log::info!("Connecting to DNS introducers");

    let client2 = client.clone();
    tokio::spawn(async move {
        loop {
            client2.find_peers(true).await;
            sleep(Duration::from_secs(5)).await;
        }
    });

    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(5)).await;
            log::info!("Currently connected to {} peers", client.len().await);
        }
    });

    while let Some(_event) = receiver.recv().await {}

    Ok(())
}
