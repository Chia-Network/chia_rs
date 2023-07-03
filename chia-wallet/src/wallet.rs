use std::sync::Arc;

use chia_client::{Peer, PeerEvent};
use tokio::{sync::broadcast, task::JoinHandle};

use crate::WalletEvent;

pub struct Wallet {
    peer: Arc<Peer>,
    event_sender: broadcast::Sender<WalletEvent>,
    peer_handler: JoinHandle<()>,
}

impl Wallet {
    pub fn new(peer: Arc<Peer>) -> Self {
        let (event_sender, _) = broadcast::channel(32);
        let peer_receiver = peer.subscribe();

        let peer_handler = tokio::spawn(Self::peer_handler(peer_receiver));

        Self {
            peer,
            event_sender,
            peer_handler,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WalletEvent> {
        self.event_sender.subscribe()
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
}

impl Drop for Wallet {
    fn drop(&mut self) {
        self.peer_handler.abort();
    }
}
