use std::sync::Arc;

use chia_client::{Peer, PeerEvent};
use chia_protocol::{CoinState, RegisterForPhUpdates, RespondToPhUpdates};
use tokio::sync::{broadcast, RwLock};

use crate::{CoinStore, KeyStore, WalletEvent};

pub struct WalletHandler {
    pub(crate) peer: Arc<Peer>,
    pub(crate) key_store: Arc<RwLock<KeyStore>>,
    pub(crate) coin_store: Arc<RwLock<CoinStore>>,
    pub(crate) peer_receiver: broadcast::Receiver<PeerEvent>,
    pub(crate) event_sender: broadcast::Sender<WalletEvent>,
}

impl WalletHandler {
    pub async fn run(mut self) {
        let first = self.key_store.write().await.derive_next();
        let response = self
            .peer
            .request::<_, RespondToPhUpdates>(RegisterForPhUpdates::new(vec![first.into()], 0))
            .await
            .unwrap();
        let updates = self.filter_coin_state(response.coin_states).await;
        self.coin_store.write().await.update(updates);

        self.event_sender
            .send(WalletEvent::SyncStatusUpdate {
                derivation_index: 1,
                is_synced: true,
            })
            .ok();

        loop {
            match self.peer_receiver.recv().await {
                Ok(event) => match event {
                    PeerEvent::CoinStateUpdate(update) => {
                        let updates = self.filter_coin_state(update.items).await;
                        self.coin_store.write().await.update(updates);
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

    pub async fn filter_coin_state(&mut self, coin_states: Vec<CoinState>) -> Vec<CoinState> {
        let key_store = self.key_store.read().await;

        coin_states
            .into_iter()
            .filter_map(|state| {
                if key_store.contains((&state.coin.puzzle_hash).into()) {
                    Some(state)
                } else {
                    None
                }
            })
            .collect()
    }
}
