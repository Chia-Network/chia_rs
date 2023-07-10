use std::sync::Arc;

use chia_client::Peer;
use tokio::{
    sync::{broadcast, RwLock},
    task::JoinHandle,
};

use crate::{CoinStore, KeyStore, WalletEvent};

mod wallet_handler;

use wallet_handler::WalletHandler;

pub struct Wallet {
    peer: Arc<Peer>,
    coin_store: Arc<RwLock<CoinStore>>,
    event_sender: broadcast::Sender<WalletEvent>,
    wallet_handler: JoinHandle<()>,
}

impl Wallet {
    pub fn new(peer: Arc<Peer>, key_store: KeyStore) -> Self {
        let (event_sender, _) = broadcast::channel(32);
        let peer_receiver = peer.subscribe();
        let coin_store = Arc::new(RwLock::new(CoinStore::default()));

        let handler = WalletHandler {
            peer: Arc::clone(&peer),
            key_store,
            coin_store: Arc::clone(&coin_store),
            peer_receiver,
            event_sender: event_sender.clone(),
        };

        let wallet_handler = tokio::spawn(handler.run());

        Self {
            peer,
            coin_store,
            event_sender,
            wallet_handler,
        }
    }

    pub async fn balance(&self) -> u64 {
        self.coin_store
            .read()
            .await
            .coin_state
            .iter()
            .filter(|state| state.spent_height.is_none())
            .fold(0, |amount, state| amount + state.coin.amount)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WalletEvent> {
        self.event_sender.subscribe()
    }
}

impl Drop for Wallet {
    fn drop(&mut self) {
        self.wallet_handler.abort();
    }
}
