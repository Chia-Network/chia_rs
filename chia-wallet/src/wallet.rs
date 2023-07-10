use std::{io::Cursor, sync::Arc};

use chia_client::Peer;
use chia_primitives::puzzles::P2_DELEGATED_OR_HIDDEN;
use chia_protocol::{
    Bytes96, Coin, CoinSpend, Program, SendTransaction, SpendBundle, Streamable, TransactionAck,
};
use clvm_utils::{curry, new_list, tree_hash};
use clvmr::{
    serde::{node_from_bytes, node_to_bytes},
    Allocator,
};
use hex::ToHex;
use hex_literal::hex;
use sha2::{digest::FixedOutput, Digest, Sha256};
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

    pub async fn next_puzzle_hash(&self) -> [u8; 32] {
        self.key_store
            .read()
            .await
            .derivations
            .first()
            .unwrap()
            .puzzle_hash
    }

    pub async fn balance(&self) -> u64 {
        self.spendable_coins()
            .await
            .iter()
            .fold(0, |amount, coin| amount + coin.amount)
    }

    pub async fn send(&mut self, puzzle_hash: &[u8; 32], amount: u64, fee: u64) -> bool {
        let key_store = self.key_store.read().await;

        let puzzle_hashes: Vec<_> = key_store
            .derivations
            .iter()
            .map(|derivation| derivation.puzzle_hash)
            .collect();

        let spendable_coins: Vec<_> = self
            .spendable_coins()
            .await
            .into_iter()
            .filter(|coin| puzzle_hashes.contains((&coin.puzzle_hash).into()))
            .collect();

        let total_amount = amount + fee;

        let selected_coins = select_coins(spendable_coins, total_amount);
        if selected_coins.is_empty() {
            return false;
        }

        let selected_amount = selected_coins
            .iter()
            .fold(0, |amount, coin| amount + coin.amount);

        let mut coin_spends = Vec::new();
        let mut signatures = Vec::new();

        let mut a = Allocator::new();

        let p2 = node_from_bytes(&mut a, &P2_DELEGATED_OR_HIDDEN).unwrap();

        for (i, coin) in selected_coins.into_iter().enumerate() {
            let derivation = key_store.derivation((&coin.puzzle_hash).into()).unwrap();
            let mut conditions = Vec::new();

            if i == 0 {
                let code_ptr = a.new_number(51.into()).unwrap();
                let ph_ptr = a.new_atom(puzzle_hash).unwrap();
                let amount_ptr = a.new_number(amount.into()).unwrap();
                conditions.push(new_list(&mut a, &[code_ptr, ph_ptr, amount_ptr]).unwrap());

                if selected_amount > total_amount {
                    let change_amount = selected_amount - total_amount;
                    let change_ph = self.next_puzzle_hash().await;

                    let ph_ptr = a.new_atom(&change_ph).unwrap();
                    let amount_ptr = a.new_number(change_amount.into()).unwrap();
                    conditions.push(new_list(&mut a, &[code_ptr, ph_ptr, amount_ptr]).unwrap());
                }
            }

            let condition_list = new_list(&mut a, &conditions).unwrap();
            let delegated_puzzle = a.new_pair(a.one(), condition_list).unwrap();

            let nil = a.null();
            let solution = new_list(&mut a, &[nil, delegated_puzzle, nil]).unwrap();
            let pk = a.new_atom(&derivation.public_key.to_bytes()).unwrap();
            let puzzle_reveal = curry(&mut a, p2, &[pk]).unwrap();

            let puzzle_bytes = node_to_bytes(&a, puzzle_reveal).unwrap();
            let puzzle_program = Program::parse(&mut Cursor::new(&puzzle_bytes)).unwrap();

            let solution_bytes = node_to_bytes(&a, solution).unwrap();
            let solution_program = Program::parse(&mut Cursor::new(&solution_bytes)).unwrap();

            dbg!(puzzle_bytes.encode_hex::<String>());
            dbg!(solution_bytes.encode_hex::<String>());

            let coin_id = coin.coin_id();
            let coin_spend = CoinSpend::new(coin, puzzle_program, solution_program);

            let delegated_puzzle_bytes = node_to_bytes(&a, delegated_puzzle).unwrap();
            dbg!(delegated_puzzle_bytes.encode_hex::<String>());

            let raw_message = tree_hash(&a, delegated_puzzle);
            let agg_sig_me_extra_data =
                hex!("ae83525ba8d1dd3f09b277de18ca3e43fc0af20d20c4b3e92ef2a48bd291ccb2");

            dbg!(raw_message.encode_hex::<String>());
            dbg!(coin_id.encode_hex::<String>());
            dbg!(agg_sig_me_extra_data.encode_hex::<String>());

            let mut message = Vec::with_capacity(96);
            message.extend(raw_message);
            message.extend(coin_id);
            message.extend(agg_sig_me_extra_data);

            let signature = derivation.secret_key.sign(&message);

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

    pub async fn spendable_coins(&self) -> Vec<Coin> {
        self.coin_store
            .read()
            .await
            .coin_state
            .iter()
            .filter_map(|state| {
                if state.spent_height.is_none() {
                    Some(state.coin.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    // pub async fn balance(&self) -> u64 {
    //     self.coin_store
    //         .read()
    //         .await
    //         .coin_state
    //         .iter()
    //         .filter(|state| state.spent_height.is_none())
    //         .fold(0, |amount, state| amount + state.coin.amount)
    // }

    pub fn subscribe(&self) -> broadcast::Receiver<WalletEvent> {
        self.event_sender.subscribe()
    }
}

impl Drop for Wallet {
    fn drop(&mut self) {
        self.wallet_handler.abort();
    }
}
