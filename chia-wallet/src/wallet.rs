use std::sync::Arc;

use chia_client::{Peer, PeerEvent};
use chia_protocol::{RegisterForPhUpdates, RespondToPhUpdates};
use tokio::{
    sync::{broadcast, RwLock},
    task::JoinHandle,
};

use crate::{CoinStore, KeyStore, WalletEvent};

pub struct Wallet {
    peer: Arc<Peer>,
    coin_store: Arc<RwLock<CoinStore>>,
    pending_spent: Vec<[u8; 32]>,
    event_sender: broadcast::Sender<WalletEvent>,
    peer_event_handler: JoinHandle<()>,
}

impl Wallet {
    pub fn new(peer: Arc<Peer>, key_store: KeyStore) -> Self {
        let (event_sender, _) = broadcast::channel(32);
        let peer_receiver = peer.subscribe();

        let coin_store = Arc::new(RwLock::new(CoinStore::default()));

        let peer_event_handler = tokio::spawn(handle_peer_events(
            Arc::clone(&peer),
            peer_receiver,
            Arc::clone(&coin_store),
            key_store,
        ));

        Self {
            peer,
            coin_store,
            pending_spent: Vec::new(),
            event_sender,
            peer_event_handler,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WalletEvent> {
        self.event_sender.subscribe()
    }
}

impl Drop for Wallet {
    fn drop(&mut self) {
        self.peer_event_handler.abort();
    }
}

async fn handle_peer_events(
    peer: Arc<Peer>,
    mut peer_receiver: broadcast::Receiver<PeerEvent>,
    coin_store: Arc<RwLock<CoinStore>>,
    mut key_store: KeyStore,
) {
    let first = key_store.add_next();
    let response = peer
        .request::<_, RespondToPhUpdates>(RegisterForPhUpdates::new(vec![first.into()], 0))
        .await
        .unwrap();

    coin_store.write().await.update(response.coin_states);

    loop {
        match peer_receiver.recv().await {
            Ok(event) => match event {
                PeerEvent::CoinStateUpdate(update) => {
                    coin_store.write().await.update(update.items);
                }
                PeerEvent::NewPeakWallet(_) => {}
            },
            Err(broadcast::error::RecvError::Closed) => {
                break;
            }
            _ => {}
        }
    }
}
