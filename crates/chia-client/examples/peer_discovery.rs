use std::{net::SocketAddr, time::Duration};

use chia_client::{create_tls_connector, Peer};
use chia_protocol::{Handshake, NodeType, ProtocolMessageTypes};
use chia_ssl::ChiaCertificate;
use chia_traits::Streamable;
use dns_lookup::lookup_host;
use tokio::time::timeout;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let cert = ChiaCertificate::generate()?;
    let tls = create_tls_connector(cert.cert_pem.as_bytes(), cert.key_pem.as_bytes())?;

    for ip in lookup_host("dns-introducer.chia.net")? {
        let Ok(response) = timeout(
            Duration::from_secs(3),
            Peer::connect(SocketAddr::new(ip, 8444), tls.clone()),
        )
        .await
        else {
            log::info!("{ip} exceeded connection timeout of 3 seconds");
            continue;
        };

        let (peer, mut receiver) = response?;

        peer.send(Handshake {
            network_id: "mainnet".to_string(),
            protocol_version: "0.0.37".to_string(),
            software_version: "0.0.0".to_string(),
            server_port: 0,
            node_type: NodeType::Wallet,
            capabilities: vec![
                (1, "1".to_string()),
                (2, "1".to_string()),
                (3, "1".to_string()),
            ],
        })
        .await?;

        let Ok(message) = timeout(Duration::from_secs(1), receiver.recv()).await else {
            log::info!("{ip} exceeded timeout of 1 second");
            continue;
        };

        let Some(message) = message else {
            log::info!("{ip} did not send any messages");
            continue;
        };

        if message.msg_type != ProtocolMessageTypes::Handshake {
            log::info!("{ip} sent an unexpected message {:?}", message.msg_type);
            continue;
        }

        let Ok(handshake) = Handshake::from_bytes(&message.data) else {
            log::info!("{ip} sent an invalid handshake");
            continue;
        };

        log::info!(
            "{ip} handshake sent with protocol version {}",
            handshake.protocol_version
        );
    }

    Ok(())
}
