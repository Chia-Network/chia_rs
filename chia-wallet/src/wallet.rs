use std::{io::Cursor, sync::Arc};

use chia_client::Peer;
use chia_primitives::puzzles::P2_DELEGATED_OR_HIDDEN;
use chia_protocol::{
    Bytes96, CoinSpend, Program, SendTransaction, SpendBundle, Streamable, TransactionAck,
};
use clvm_utils::{curry, new_list, tree_hash};
use clvmr::{
    serde::{node_from_bytes, node_to_bytes},
    Allocator,
};
use hex::ToHex;
use hex_literal::hex;
use tokio::{
    sync::{broadcast, RwLock},
    task::JoinHandle,
};

use crate::{select_coins, CoinStore, KeyStore, WalletEvent};

mod wallet_handler;

use wallet_handler::WalletHandler;

pub struct Wallet {
    peer: Arc<Peer>,
    key_store: Arc<RwLock<KeyStore>>,
    coin_store: Arc<RwLock<CoinStore>>,
    event_sender: broadcast::Sender<WalletEvent>,
    wallet_handler: JoinHandle<()>,
}

impl Wallet {
    pub fn new(peer: Arc<Peer>, key_store: KeyStore) -> Self {
        let (event_sender, _) = broadcast::channel(32);
        let peer_receiver = peer.subscribe();
        let coin_store = Arc::new(RwLock::new(CoinStore::default()));
        let key_store = Arc::new(RwLock::new(key_store));

        let handler = WalletHandler {
            peer: Arc::clone(&peer),
            key_store: Arc::clone(&key_store),
            coin_store: Arc::clone(&coin_store),
            peer_receiver,
            event_sender: event_sender.clone(),
        };

        let wallet_handler = tokio::spawn(handler.run());

        Self {
            peer,
            key_store,
            coin_store,
            event_sender,
            wallet_handler,
        }
    }

    pub async fn next_puzzle_hash(&self) -> Option<[u8; 32]> {
        let key_store = self.key_store.read().await;
        let coin_store = self.coin_store.read().await;
        for puzzle_hash in key_store.puzzle_hashes() {
            if coin_store.is_used(&puzzle_hash) {
                return Some(puzzle_hash);
            }
        }
        None
    }

    pub async fn send(&self, puzzle_hash: &[u8; 32], amount: u64, fee: u64) -> bool {
        let key_store = self.key_store.read().await;

        let spendable = self.coin_store.read().await.unspent();
        let total_amount = amount + fee;

        let selected = select_coins(spendable, total_amount);
        if selected.is_empty() {
            return false;
        }

        let selected_amount = selected
            .iter()
            .fold(0, |amount, record| amount + record.coin.amount);

        let mut coin_spends = Vec::new();
        let mut signatures = Vec::new();

        let mut a = Allocator::new();

        let p2 = node_from_bytes(&mut a, &P2_DELEGATED_OR_HIDDEN).unwrap();

        for (i, record) in selected.into_iter().enumerate() {
            let secret_key = key_store
                .derivation((&record.coin.puzzle_hash).into())
                .unwrap();
            let mut conditions = Vec::new();

            if i == 0 {
                let code_ptr = a.new_number(51.into()).unwrap();
                let ph_ptr = a.new_atom(puzzle_hash).unwrap();
                let amount_ptr = a.new_number(amount.into()).unwrap();
                conditions.push(new_list(&mut a, &[code_ptr, ph_ptr, amount_ptr]).unwrap());

                if selected_amount > total_amount {
                    let change_amount = selected_amount - total_amount;
                    let change_ph = self.next_puzzle_hash().await.unwrap();

                    let ph_ptr = a.new_atom(&change_ph).unwrap();
                    let amount_ptr = a.new_number(change_amount.into()).unwrap();
                    conditions.push(new_list(&mut a, &[code_ptr, ph_ptr, amount_ptr]).unwrap());
                }
            }

            let condition_list = new_list(&mut a, &conditions).unwrap();
            let delegated_puzzle = a.new_pair(a.one(), condition_list).unwrap();

            let nil = a.null();
            let solution = new_list(&mut a, &[nil, delegated_puzzle, nil]).unwrap();
            let pk = a.new_atom(&secret_key.to_public_key().to_bytes()).unwrap();
            let puzzle_reveal = curry(&mut a, p2, &[pk]).unwrap();

            let puzzle_bytes = node_to_bytes(&a, puzzle_reveal).unwrap();
            let puzzle_program = Program::parse(&mut Cursor::new(&puzzle_bytes)).unwrap();

            let solution_bytes = node_to_bytes(&a, solution).unwrap();
            let solution_program = Program::parse(&mut Cursor::new(&solution_bytes)).unwrap();

            let coin_id = record.coin.coin_id();
            let coin_spend = CoinSpend::new(record.coin.clone(), puzzle_program, solution_program);

            let raw_message = tree_hash(&a, delegated_puzzle);
            let agg_sig_me_extra_data =
                hex!("ae83525ba8d1dd3f09b277de18ca3e43fc0af20d20c4b3e92ef2a48bd291ccb2");

            let mut message = Vec::with_capacity(96);
            message.extend(raw_message);
            message.extend(coin_id);
            message.extend(agg_sig_me_extra_data);

            let signature = secret_key.sign(&message);

            coin_spends.push(coin_spend);
            signatures.push(signature);
        }

        let spend_bundle = SpendBundle::new(
            coin_spends,
            Bytes96::from(
                &signatures
                    .into_iter()
                    .reduce(|a, b| a.add(&b))
                    .unwrap()
                    .to_bytes(),
            ),
        );

        let result: TransactionAck = self
            .peer
            .request(SendTransaction::new(spend_bundle))
            .await
            .unwrap();

        dbg!(result);

        true
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
