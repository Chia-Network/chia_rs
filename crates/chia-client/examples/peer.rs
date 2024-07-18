use std::{env, net::SocketAddr};

use chia_client::{create_tls_connector, Peer};
use chia_protocol::{Handshake, NodeType};
use chia_ssl::ChiaCertificate;
use chia_traits::Streamable;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ssl = ChiaCertificate::generate()?;
    let tls_connector = create_tls_connector(ssl.cert_pem.as_bytes(), ssl.key_pem.as_bytes())?;
    let (peer, mut receiver) = Peer::connect(
        SocketAddr::new(env::var("PEER")?.parse()?, 58444),
        tls_connector,
    )
    .await?;

    peer.send(Handshake {
        network_id: "testnet11".to_string(),
        protocol_version: "0.0.34".to_string(),
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

    let message = receiver.recv().await.unwrap();
    let handshake = Handshake::from_bytes(&message.data)?;
    println!("{handshake:#?}");

    while let Some(message) = receiver.recv().await {
        println!("{message:?}");
    }

    Ok(())
}
