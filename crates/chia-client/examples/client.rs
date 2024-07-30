use std::{sync::Arc, time::Duration};

use chia_client::{create_tls_connector, Client, ClientOptions, Event, Network};
use chia_protocol::{NewPeakWallet, NodeType, ProtocolMessageTypes};
use chia_ssl::ChiaCertificate;
use chia_traits::Streamable;
use tokio::{sync::Mutex, time::sleep};

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
            target_peers: 1000,
            connection_concurrency: 50,
            node_type: NodeType::Wallet,
            capabilities: vec![
                (1, "1".to_string()),
                (2, "1".to_string()),
                (3, "1".to_string()),
            ],
            protocol_version: "0.0.34".parse()?,
            software_version: "0.0.0".to_string(),
            connection_timeout: Duration::from_secs(5),
            handshake_timeout: Duration::from_secs(5),
            request_peers_timeout: Duration::from_secs(5),
        },
    );

    log::info!("Connecting to DNS introducers");

    let clone = client.clone();
    tokio::spawn(async move {
        loop {
            clone.discover_peers(true).await;
            sleep(Duration::from_secs(5)).await;
        }
    });

    let height = Arc::new(Mutex::new(0));
    let height_clone = height.clone();

    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(1)).await;
            log::info!(
                "Currently connected to {} peers, with peak height {}",
                client.lock().await.peers().len(),
                *height_clone.lock().await
            );
        }
    });

    while let Some(event) = receiver.recv().await {
        let Event::Message(_peer_id, message) = event else {
            continue;
        };

        if message.msg_type != ProtocolMessageTypes::NewPeakWallet {
            continue;
        }

        let Ok(new_peak) = NewPeakWallet::from_bytes(&message.data) else {
            continue;
        };

        let mut height = height.lock().await;

        if new_peak.height > *height {
            *height = new_peak.height;
        }
    }

    Ok(())
}
