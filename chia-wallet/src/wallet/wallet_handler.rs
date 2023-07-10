use std::sync::Arc;

use chia_client::{Peer, PeerEvent};
use chia_protocol::{RegisterForPhUpdates, RespondToPhUpdates};
use tokio::sync::{broadcast, RwLock};

use crate::{CoinStore, KeyStore, WalletEvent};

pub struct WalletHandler {
    pub(super) peer: Arc<Peer>,
    pub(super) key_store: KeyStore,
    pub(super) coin_store: Arc<RwLock<CoinStore>>,
    pub(super) peer_receiver: broadcast::Receiver<PeerEvent>,
    pub(super) event_sender: broadcast::Sender<WalletEvent>,
}

impl WalletHandler {
    pub async fn run(mut self) {
        let first = self.key_store.add_next();
        let response = self
            .peer
            .request::<_, RespondToPhUpdates>(RegisterForPhUpdates::new(vec![first.into()], 0))
            .await
            .unwrap();

        self.coin_store.write().await.update(response.coin_states);

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
                        self.coin_store.write().await.update(update.items);
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
}
