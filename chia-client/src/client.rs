use tokio_tungstenite::Connector;

use crate::Peer;

pub struct ClientOptions {
    pub connector: Connector,
}

pub struct Client {
    peers: Vec<Peer>,
}

impl Client {
    pub fn new() -> Self {
        Self { peers: Vec::new() }
    }
}
