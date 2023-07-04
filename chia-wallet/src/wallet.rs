use std::sync::Arc;

use chia_client::{Peer, PeerEvent};
use tokio::{sync::broadcast, task::JoinHandle};

use crate::{KeyStore, WalletEvent};

pub struct Wallet {
    peer: Arc<Peer>,
    key_store: KeyStore,
    event_sender: broadcast::Sender<WalletEvent>,
    peer_handler: JoinHandle<()>,
}

impl Wallet {
    pub fn new(peer: Arc<Peer>, key_store: KeyStore) -> Self {
        let (event_sender, _) = broadcast::channel(32);
        let peer_receiver = peer.subscribe();

        let peer_handler = tokio::spawn(Self::peer_handler(peer_receiver));

        Self {
            peer,
            key_store,
            event_sender,
            peer_handler,
        }
    }

    async fn peer_handler(mut peer_receiver: broadcast::Receiver<PeerEvent>) {
        loop {
            match peer_receiver.recv().await {
                Ok(event) => {
                    dbg!(event);
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
                _ => {}
            }
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WalletEvent> {
        self.event_sender.subscribe()
    }
}

impl Drop for Wallet {
    fn drop(&mut self) {
        self.peer_handler.abort();
    }
}
