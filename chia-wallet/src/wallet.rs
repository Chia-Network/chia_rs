use std::sync::Arc;

use chia_client::{Peer, PeerEvent};
use chia_protocol::{Coin, CoinState};
use tokio::{sync::broadcast, task::JoinHandle};

use crate::{select_coins, KeyStore, WalletEvent};

pub struct Wallet {
    peer: Arc<Peer>,
    coin_state: Vec<CoinState>,
    pending_coins: Vec<[u8; 32]>,
    event_sender: broadcast::Sender<WalletEvent>,
    peer_event_handler: JoinHandle<()>,
}

impl Wallet {
    pub fn new(peer: Arc<Peer>, key_store: KeyStore) -> Self {
        let (event_sender, _) = broadcast::channel(32);
        let peer_receiver = peer.subscribe();

        let peer_event_handler = tokio::spawn(handle_peer_events(peer_receiver));

        Self {
            peer,
            coin_state: Vec::new(),
            pending_coins: Vec::new(),
            event_sender,
            peer_event_handler,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WalletEvent> {
        self.event_sender.subscribe()
    }

    pub fn spendable_coins(&self) -> Vec<&Coin> {
        self.coin_state
            .iter()
            .filter_map(|coin_state| {
                if self.pending_coins.contains(&coin_state.coin.coin_id()) {
                    Some(&coin_state.coin)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn select_coins(&self, amount: u64) -> Vec<&Coin> {
        select_coins(self.spendable_coins(), amount)
    }
}

impl Drop for Wallet {
    fn drop(&mut self) {
        self.peer_event_handler.abort();
    }
}

async fn handle_peer_events(mut peer_receiver: broadcast::Receiver<PeerEvent>) {
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
