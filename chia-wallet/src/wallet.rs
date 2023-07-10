use std::sync::Arc;

use chia_client::{Peer, PeerEvent};
use chia_protocol::{Coin, CoinState, RegisterForPhUpdates, RespondToPhUpdates};
use tokio::{
    sync::{broadcast, RwLock},
    task::JoinHandle,
};

use crate::{select_coins, KeyStore, WalletEvent};

pub struct Wallet {
    peer: Arc<Peer>,
    coin_state: Arc<RwLock<Vec<CoinState>>>,
    pending_spent: Vec<[u8; 32]>,
    event_sender: broadcast::Sender<WalletEvent>,
    peer_event_handler: JoinHandle<()>,
}

impl Wallet {
    pub fn new(peer: Arc<Peer>, key_store: KeyStore) -> Self {
        let (event_sender, _) = broadcast::channel(32);
        let peer_receiver = peer.subscribe();

        let coin_state = Arc::new(RwLock::new(Vec::new()));

        let peer_event_handler = tokio::spawn(handle_peer_events(
            Arc::clone(&peer),
            peer_receiver,
            Arc::clone(&coin_state),
            key_store,
        ));

        Self {
            peer,
            coin_state,
            pending_spent: Vec::new(),
            event_sender,
            peer_event_handler,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WalletEvent> {
        self.event_sender.subscribe()
    }

    pub fn spendable_coins(&self) -> Vec<Coin> {
        self.coin_state
            .blocking_read()
            .iter()
            .filter_map(|coin_state| {
                if coin_state.spent_height.is_none()
                    && !self.pending_spent.contains(&coin_state.coin.coin_id())
                {
                    Some(coin_state.coin.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn select_coins(&self, amount: u64) -> Vec<Coin> {
        select_coins(self.spendable_coins(), amount)
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
    coin_state: Arc<RwLock<Vec<CoinState>>>,
    mut key_store: KeyStore,
) {
    let first = key_store.add_next();
    let response = peer
        .request::<_, RespondToPhUpdates>(RegisterForPhUpdates::new(vec![first.into()], 0))
        .await
        .unwrap();

    handle_coin_state_update(Arc::clone(&coin_state), response.coin_states).await;

    loop {
        match peer_receiver.recv().await {
            Ok(event) => match event {
                PeerEvent::CoinStateUpdate(update) => {
                    handle_coin_state_update(Arc::clone(&coin_state), update.items).await;
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

async fn handle_coin_state_update(
    coin_state: Arc<RwLock<Vec<CoinState>>>,
    update_items: Vec<CoinState>,
) {
    for updated_item in update_items {
        let mut coin_state_lock = coin_state.write().await;
        match coin_state_lock
            .iter_mut()
            .find(|item| item.coin == updated_item.coin)
        {
            Some(existing) => *existing = updated_item,
            None => coin_state_lock.push(updated_item),
        }
    }
}
