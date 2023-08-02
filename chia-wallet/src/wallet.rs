use std::{io::Cursor, sync::Arc};

use chia_bls::{SecretKey, Signature};
use chia_client::{Peer, PeerEvent};
use chia_primitives::{
    puzzles::{
        CAT_PUZZLE, CAT_PUZZLE_HASH, EVERYTHING_WITH_SIGNATURE_TAIL_PUZZLE, LAUNCHER_PUZZLE,
        LAUNCHER_PUZZLE_HASH, NFT_INTERMEDIATE_LAUNCHER_PUZZLE, NFT_METADATA_UPDATER_PUZZLE_HASH,
        NFT_OWNERSHIP_LAYER_PUZZLE, NFT_OWNERSHIP_LAYER_PUZZLE_HASH, NFT_ROYALTY_TRANSFER_PUZZLE,
        NFT_STATE_LAYER_PUZZLE, NFT_STATE_LAYER_PUZZLE_HASH, SETTLEMENT_PAYMENTS_PUZZLE,
        SETTLEMENT_PAYMENTS_PUZZLE_HASH, SINGLETON_PUZZLE, STANDARD_PUZZLE,
    },
    sign_agg_sig_me, CatArgs, CatSolution, CoinProof, Condition, DidSolution, EveProof,
    EverythingWithSignatureTailArgs, LauncherSolution, NftIntermediateLauncherArgs,
    NftOwnershipLayerArgs, NftOwnershipLayerSolution, NftRoyaltyTransferPuzzleArgs,
    NftStateLayerArgs, NftStateLayerSolution, Proof, SingletonArgs, SingletonSolution,
    SingletonStruct, StandardArgs, StandardSolution,
};
use chia_protocol::{Coin, CoinSpend, CoinState, Program, SpendBundle};
use chia_traits::Streamable;
use clvm_utils::{clvm_list, clvm_quote, curry, tree_hash, FromClvm, LazyNode, ToClvm};
use clvmr::{allocator::NodePtr, serde::node_from_bytes, Allocator};
use sha2::{digest::FixedOutput, Digest, Sha256};
use tokio::{
    sync::{
        broadcast::{self, Receiver},
        RwLock,
    },
    task::{JoinError, JoinHandle},
};

mod cat_info;
mod did_info;
mod key_store;
mod nft_info;
mod nft_mint;
mod offer;
mod wallet_event;
mod wallet_state;

use wallet_state::*;

pub use cat_info::*;
pub use did_info::*;
pub use key_store::*;
pub use nft_info::*;
pub use nft_mint::*;
pub use offer::*;
pub use wallet_event::*;

use crate::utils::int_to_bytes;

pub struct Wallet {
    peer: Arc<Peer>,
    event_receiver: Receiver<WalletEvent>,
    pub state: Arc<RwLock<WalletState>>,
    runner_handle: Option<JoinHandle<()>>,
}

impl Wallet {
    pub fn new(peer: Arc<Peer>, key_store: KeyStore) -> Self {
        let (event_sender, event_receiver) = broadcast::channel(32);

        let state = Arc::new(RwLock::new(WalletState::new(
            Arc::clone(&peer),
            key_store,
            event_sender,
        )));

        let peer_receiver = peer.subscribe();

        let runner_handle = Some(tokio::spawn(Self::run(Arc::clone(&state), peer_receiver)));

        Self {
            peer,
            event_receiver,
            state,
            runner_handle,
        }
    }

    async fn run(state: Arc<RwLock<WalletState>>, mut peer_receiver: Receiver<PeerEvent>) {
        state.write().await.unused_puzzle_hash().await.ok();

        while let Ok(event) = peer_receiver.recv().await {
            if let PeerEvent::CoinStateUpdate(update) = event {
                let mut state = state.write().await;
                state.update_unknown_coins(update.items).await;
            }
        }
    }

    pub fn subscribe(&self) -> Receiver<WalletEvent> {
        self.event_receiver.resubscribe()
    }

    pub async fn unused_puzzle_hash(&self) -> anyhow::Result<[u8; 32]> {
        self.state.write().await.unused_puzzle_hash().await
    }

    pub async fn offer_royalty_enabled_nft_for_cat(
        &self,
        nft_id: &[u8; 32],
        asset_id: &[u8; 32],
        requested_amount: u64,
    ) -> anyhow::Result<SpendBundle> {
        // Get NFT info.
        let nft_info = self
            .state
            .read()
            .await
            .get_nft_info(nft_id)
            .ok_or(anyhow::Error::msg("could not find NFT info"))?;

        // Initialize the allocator.
        let mut a = Allocator::new();

        let offer_mod = node_from_bytes(&mut a, &SETTLEMENT_PAYMENTS_PUZZLE)?;
        let cat_mod = node_from_bytes(&mut a, &CAT_PUZZLE)?;

        // Calculate requested payment.
        let puzzle_hash = self.unused_puzzle_hash().await?;

        let requested_payment = Condition::CreateCoin {
            puzzle_hash,
            amount: requested_amount as i64,
            memos: vec![puzzle_hash],
        };

        // Calculate requested settlement puzzle hash.
        let settlement_args = CatArgs {
            mod_hash: CAT_PUZZLE_HASH,
            tail_program_hash: *asset_id,
            inner_puzzle: LazyNode(offer_mod),
        }
        .to_clvm(&mut a)?;

        let settlement = curry(&mut a, cat_mod, settlement_args)?;
        let settlement_puzzle_hash = tree_hash(&a, settlement);

        // Calculate nonce.
        let sorted_coin_list = clvm_list!(nft_info.coin_state.coin.clone()).to_clvm(&mut a)?;
        let nonce = tree_hash(&a, sorted_coin_list);

        // Collect announcements.
        let mut announcements_to_assert = Vec::new();

        // Create spends.
        let mut coin_spends: Vec<CoinSpend> = Vec::new();
        let mut signatures: Vec<Signature> = Vec::new();

        // Add requested CAT.
        let requested_payment_ptr = requested_payment.to_clvm(&mut a)?;
        let requested_payment_args = <(LazyNode, LazyNode)>::from_clvm(&a, requested_payment_ptr)?;
        let inner_solutions =
            clvm_list!((nonce, clvm_list!(requested_payment_args.1))).to_clvm(&mut a)?;

        let requested_coin_spend = CoinSpend::new(
            Coin::new([0; 32].into(), settlement_puzzle_hash.into(), 0),
            Program::from_clvm(&a, settlement)?,
            Program::from_clvm(&a, inner_solutions)?,
        );

        coin_spends.push(requested_coin_spend);

        // Calculate puzzle announcement.
        let payment_message = (nonce, vec![requested_payment_args.1]).to_clvm(&mut a)?;
        let payment_message_hash = tree_hash(&a, payment_message);

        let mut hasher = Sha256::new();
        hasher.update(settlement_puzzle_hash);
        hasher.update(payment_message_hash);
        let payment_announcement_id: [u8; 32] = hasher.finalize_fixed().into();

        announcements_to_assert.push(Condition::AssertPuzzleAnnouncement {
            announcement_id: payment_announcement_id,
        });

        // Spend NFT.
        let mut nft_condition_list = vec![Condition::CreateCoin {
            puzzle_hash: SETTLEMENT_PAYMENTS_PUZZLE_HASH,
            amount: nft_info.coin_state.coin.amount as i64,
            memos: vec![SETTLEMENT_PAYMENTS_PUZZLE_HASH],
        }];

        nft_condition_list.extend(announcements_to_assert);

        let (nft_coin_spend, nft_signature, _) = self
            .spend_nft(nft_id, NewOwner::Reset, nft_condition_list)
            .await?;

        coin_spends.push(nft_coin_spend);
        signatures.push(nft_signature);

        // Construct spend bundle.
        let spend_bundle = SpendBundle::new(
            coin_spends,
            signatures
                .into_iter()
                .reduce(|aggregate, signature| aggregate.add(&signature))
                .unwrap()
                .to_bytes()
                .into(),
        );

        Ok(spend_bundle)
    }

    pub async fn issue_cat_with_key(
        &self,
        secret_key: &SecretKey,
        target_puzzle_hash: [u8; 32],
        issue_amount: u64,
        fee: u64,
    ) -> anyhow::Result<(Vec<CoinSpend>, Signature)> {
        let mut coin_spends = Vec::new();
        let mut signatures = Vec::new();

        // Select coins and calculate amounts.
        let required_amount = issue_amount + fee;
        let selected_coins = self
            .state
            .read()
            .await
            .select_standard_coins(required_amount);

        let funding_coin = selected_coins
            .first()
            .ok_or(anyhow::Error::msg("missing funding coin"))?
            .clone();

        // Calculate change.
        let selected_amount = selected_coins
            .iter()
            .fold(0, |amount, coin| amount + coin.amount);

        let change_amount = selected_amount - required_amount;
        let change_puzzle_hash = self.unused_puzzle_hash().await?;

        // Initialize the allocator and puzzles.
        let mut a = Allocator::new();

        let tail_mod = node_from_bytes(&mut a, &EVERYTHING_WITH_SIGNATURE_TAIL_PUZZLE)?;
        let p2_mod = node_from_bytes(&mut a, &STANDARD_PUZZLE)?;
        let cat_mod = node_from_bytes(&mut a, &CAT_PUZZLE)?;

        // Construct the TAIL puzzle and solution.
        let tail_args = EverythingWithSignatureTailArgs {
            public_key: secret_key.to_public_key(),
        }
        .to_clvm(&mut a)?;

        let tail = curry(&mut a, tail_mod, tail_args)?;

        let asset_id = tree_hash(&a, tail);

        let tail_solution = a.null();

        // Construct the CAT puzzle.
        let tail_condition =
            clvm_list!(51, (), -113, LazyNode(tail), LazyNode(tail_solution)).to_clvm(&mut a)?;
        let issuance_condition_list = vec![Condition::CreateCoin {
            puzzle_hash: target_puzzle_hash,
            amount: issue_amount as i64,
            memos: vec![target_puzzle_hash],
        }];
        let issuance_conditions =
            clvm_quote!((LazyNode(tail_condition), issuance_condition_list)).to_clvm(&mut a)?;
        let issuance_conditions_tree_hash = tree_hash(&a, issuance_conditions);

        let cat_args = CatArgs {
            mod_hash: CAT_PUZZLE_HASH,
            tail_program_hash: asset_id,
            inner_puzzle: LazyNode(issuance_conditions),
        }
        .to_clvm(&mut a)?;

        let cat = curry(&mut a, cat_mod, cat_args)?;
        let cat_puzzle_hash = tree_hash(&a, cat);

        // Create the fee coin spends.
        for (i, coin) in selected_coins.into_iter().enumerate() {
            // Fetch the key pair.
            let secret_key = self
                .state
                .read()
                .await
                .key_store
                .secret_key_of((&coin.puzzle_hash).into())
                .ok_or(anyhow::Error::msg("missing secret key for fee coin spend"))?
                .clone();
            let public_key = secret_key.to_public_key();

            // Construct the p2 puzzle.
            let fee_p2_args = StandardArgs {
                synthetic_key: public_key,
            }
            .to_clvm(&mut a)?;
            let fee_p2 = curry(&mut a, p2_mod, fee_p2_args)?;

            // Calculate the conditions.
            let mut condition_list = Vec::new();

            if i == 0 {
                // Create CAT coin.
                condition_list.push(Condition::CreateCoin {
                    puzzle_hash: cat_puzzle_hash,
                    amount: issue_amount as i64,
                    memos: vec![],
                });

                // Create change coin.
                if change_amount > 0 {
                    condition_list.push(Condition::CreateCoin {
                        puzzle_hash: change_puzzle_hash,
                        amount: change_amount as i64,
                        memos: vec![],
                    });
                }
            }

            // Construct the solution.
            let conditions = clvm_quote!(condition_list).to_clvm(&mut a)?;
            let conditions_tree_hash = tree_hash(&a, conditions);
            let fee_p2_solution =
                StandardSolution::with_conditions(&mut a, conditions).to_clvm(&mut a)?;

            let signature = sign_agg_sig_me(
                &secret_key,
                &conditions_tree_hash,
                &coin.coin_id(),
                &self.peer.network.agg_sig_me_extra_data,
            );

            // Construct coin spend.
            let coin_spend = CoinSpend::new(
                coin,
                Program::from_clvm(&a, fee_p2)?,
                Program::from_clvm(&a, fee_p2_solution)?,
            );

            coin_spends.push(coin_spend);
            signatures.push(signature);
        }

        // Create the eve CAT coin.
        let eve_coin = Coin::new(
            funding_coin.coin_id().into(),
            cat_puzzle_hash.into(),
            issue_amount,
        );

        let next_coin_proof = CoinProof {
            parent_coin_info: funding_coin.coin_id(),
            inner_puzzle_hash: issuance_conditions_tree_hash,
            amount: issue_amount,
        };

        // Construct the CAT solution.
        let cat_solution = CatSolution {
            inner_puzzle_solution: LazyNode(a.null()),
            lineage_proof: None,
            prev_coin_id: eve_coin.coin_id(),
            this_coin_info: eve_coin.clone(),
            next_coin_proof,
            prev_subtotal: 0,
            extra_delta: 0,
        }
        .to_clvm(&mut a)?;

        // Construct the coin spend.
        let coin_spend = CoinSpend::new(
            eve_coin.clone(),
            Program::from_clvm(&a, cat)?,
            Program::from_clvm(&a, cat_solution)?,
        );

        coin_spends.push(coin_spend);

        // Sign the delta.
        let signature = sign_agg_sig_me(
            secret_key,
            &[],
            &eve_coin.coin_id(),
            &self.peer.network.agg_sig_me_extra_data,
        );

        signatures.push(signature);

        Ok((
            coin_spends,
            signatures
                .into_iter()
                .reduce(|aggregate, signature| aggregate.add(&signature))
                .unwrap(),
        ))
    }

    pub async fn send_cat(
        &self,
        asset_id: &[u8; 32],
        target_puzzle_hash: [u8; 32],
        send_amount: u64,
        fee: u64,
    ) -> anyhow::Result<(Vec<CoinSpend>, Signature)> {
        let mut coin_spends = Vec::new();
        let mut signatures = Vec::new();

        // Initialize the allocator and puzzles.
        let mut a = Allocator::new();

        let p2_mod = node_from_bytes(&mut a, &STANDARD_PUZZLE)?;

        // Fee
        let selected_fee_coins = self.state.read().await.select_standard_coins(fee);

        if selected_fee_coins.is_empty() {
            return Err(anyhow::Error::msg("insufficient fee balance"));
        }

        let selected_fee_amount = selected_fee_coins
            .iter()
            .fold(0, |amount, coin| amount + coin.amount);

        let fee_change_amount = selected_fee_amount - fee;

        // CAT
        let selected_cat_coins = self
            .state
            .read()
            .await
            .select_cat_coins(asset_id, send_amount);

        if selected_cat_coins.is_empty() {
            return Err(anyhow::Error::msg("insufficient CAT balance"));
        }

        let selected_cat_amount = selected_cat_coins.iter().fold(0, |amount, cat_coin| {
            amount + cat_coin.coin_state.coin.amount
        });

        let change_amount = selected_cat_amount - send_amount;
        let change_puzzle_hash = self.unused_puzzle_hash().await?;

        // Fee spends
        for fee_coin in selected_fee_coins {
            // Construct the p2 puzzle.
            let secret_key = self
                .state
                .read()
                .await
                .key_store
                .secret_key_of((&fee_coin.puzzle_hash).into())
                .ok_or(anyhow::Error::msg("missing secret key for p2 spend"))?
                .clone();

            let p2_args = StandardArgs {
                synthetic_key: secret_key.to_public_key(),
            }
            .to_clvm(&mut a)?;

            let p2 = curry(&mut a, p2_mod, p2_args)?;

            // Create the conditions.
            let mut conditions = Vec::new();

            if fee_change_amount > 0 {
                conditions.push(Condition::CreateCoin {
                    puzzle_hash: change_puzzle_hash,
                    amount: fee_change_amount as i64,
                    memos: vec![],
                });
            }

            // Construct the p2 solution.
            let conditions = clvm_quote!(conditions).to_clvm(&mut a)?;
            let conditions_tree_hash = tree_hash(&a, conditions);
            let p2_solution =
                StandardSolution::with_conditions(&mut a, conditions).to_clvm(&mut a)?;

            let signature = sign_agg_sig_me(
                &secret_key,
                &conditions_tree_hash,
                &fee_coin.coin_id(),
                &self.peer.network.agg_sig_me_extra_data,
            );

            // Create coin spend
            let coin_spend = CoinSpend::new(
                fee_coin,
                Program::from_clvm(&a, p2)?,
                Program::from_clvm(&a, p2_solution)?,
            );

            coin_spends.push(coin_spend);
            signatures.push(signature);
        }

        // CAT spends
        let (cat_spends, cat_signature) = self
            .spend_cats(
                asset_id,
                &selected_cat_coins
                    .into_iter()
                    .enumerate()
                    .map(|(i, cat_coin)| CatSpend {
                        cat_coin,
                        conditions: if i == 0 {
                            let mut conditions = vec![Condition::CreateCoin {
                                puzzle_hash: target_puzzle_hash,
                                amount: send_amount as i64,
                                memos: vec![target_puzzle_hash],
                            }];

                            if change_amount > 0 {
                                conditions.push(Condition::CreateCoin {
                                    puzzle_hash: change_puzzle_hash,
                                    amount: change_amount as i64,
                                    memos: vec![change_puzzle_hash],
                                });
                            }

                            conditions
                        } else {
                            vec![]
                        },
                        extra_delta: 0,
                    })
                    .collect::<Vec<_>>(),
            )
            .await?;

        coin_spends.extend(cat_spends);
        signatures.push(cat_signature);

        Ok((
            coin_spends,
            signatures
                .into_iter()
                .reduce(|aggregate, signature| aggregate.add(&signature))
                .unwrap(),
        ))
    }

    async fn spend_cats(
        &self,
        asset_id: &[u8; 32],
        cat_spends: &[CatSpend],
    ) -> anyhow::Result<(Vec<CoinSpend>, Signature)> {
        let mut coin_spends = Vec::new();
        let mut signatures = Vec::new();

        // Initialize the allocator and puzzles.
        let mut a = Allocator::new();

        let p2_mod = node_from_bytes(&mut a, &STANDARD_PUZZLE)?;
        let cat_mod = node_from_bytes(&mut a, &CAT_PUZZLE)?;

        // Create the CAT coin spends.
        let mut total_delta = 0;

        for (index, cat_spend) in cat_spends.iter().enumerate() {
            // Calculate the delta and add it to the subtotal.
            let delta =
                cat_spend
                    .conditions
                    .iter()
                    .fold(-cat_spend.extra_delta, |delta, condition| {
                        if let Condition::CreateCoin { amount, .. } = condition {
                            if *amount != -113 {
                                return delta + amount;
                            }
                        }
                        delta
                    });

            let prev_subtotal = total_delta;

            total_delta += delta;

            // Find information of neighboring coins on the ring.
            let prev_cat_coin = &cat_spends[index.wrapping_sub(1) % cat_spends.len()].cat_coin;
            let next_cat_coin = &cat_spends[index.wrapping_add(1) % cat_spends.len()].cat_coin;

            // Construct the p2 puzzle.
            let secret_key = self
                .state
                .read()
                .await
                .key_store
                .secret_key_of(&cat_spend.cat_coin.p2_puzzle_hash)
                .ok_or(anyhow::Error::msg("missing secret key for p2 spend"))?
                .clone();

            let p2_args = StandardArgs {
                synthetic_key: secret_key.to_public_key(),
            }
            .to_clvm(&mut a)?;

            let p2 = curry(&mut a, p2_mod, p2_args)?;

            // Construct the CAT puzzle.
            let cat_args = CatArgs {
                mod_hash: CAT_PUZZLE_HASH,
                tail_program_hash: *asset_id,
                inner_puzzle: LazyNode(p2),
            }
            .to_clvm(&mut a)?;

            let cat = curry(&mut a, cat_mod, cat_args)?;

            // Construct the p2 solution.
            let conditions = clvm_quote!(&cat_spend.conditions).to_clvm(&mut a)?;
            let conditions_tree_hash = tree_hash(&a, conditions);
            let p2_solution =
                StandardSolution::with_conditions(&mut a, conditions).to_clvm(&mut a)?;

            let signature = sign_agg_sig_me(
                &secret_key,
                &conditions_tree_hash,
                &cat_spend.cat_coin.coin_state.coin.coin_id(),
                &self.peer.network.agg_sig_me_extra_data,
            );

            // Construct the CAT solution.
            let next_parent_coin_info: &[u8; 32] =
                (&next_cat_coin.coin_state.coin.parent_coin_info).into();

            let next_coin_proof = CoinProof {
                parent_coin_info: *next_parent_coin_info,
                inner_puzzle_hash: next_cat_coin.p2_puzzle_hash,
                amount: next_cat_coin.coin_state.coin.amount,
            };

            let cat_solution = CatSolution {
                inner_puzzle_solution: LazyNode(p2_solution),
                lineage_proof: Some(cat_spend.cat_coin.lineage_proof.clone()),
                prev_coin_id: prev_cat_coin.coin_state.coin.coin_id(),
                this_coin_info: cat_spend.cat_coin.coin_state.coin.clone(),
                next_coin_proof,
                prev_subtotal,
                extra_delta: cat_spend.extra_delta,
            }
            .to_clvm(&mut a)?;

            // Add the spend info.
            let coin_spend = CoinSpend::new(
                cat_spend.cat_coin.coin_state.coin.clone(),
                Program::from_clvm(&a, cat)?,
                Program::from_clvm(&a, cat_solution)?,
            );

            coin_spends.push(coin_spend);
            signatures.push(signature);
        }

        Ok((
            coin_spends,
            signatures
                .into_iter()
                .reduce(|aggregate, signature| aggregate.add(&signature))
                .unwrap(),
        ))
    }

    pub async fn mint_nfts(
        &self,
        did_id: &[u8; 32],
        nft_mints: &[NftMint],
        start_index: usize,
        total_nft_count: usize,
        fee: u64,
    ) -> anyhow::Result<(Vec<CoinSpend>, Signature, Vec<[u8; 32]>)> {
        // Get DID info.
        let did_info = self
            .state
            .read()
            .await
            .get_did_info(did_id)
            .ok_or(anyhow::Error::msg("could not find DID info"))?;

        // Select coins and calculate amounts.
        let nft_amount = 1;
        let required_amount = nft_mints.len() as u64 * nft_amount + fee;
        let selected_coins = self
            .state
            .read()
            .await
            .select_standard_coins(required_amount);
        let funding_coin = selected_coins
            .first()
            .ok_or(anyhow::Error::msg("no funding coin"))?;

        // Initialize the allocator and puzzles.
        let mut a = Allocator::new();

        let intermediate_launcher_mod = node_from_bytes(&mut a, &NFT_INTERMEDIATE_LAUNCHER_PUZZLE)?;
        let transfer_program_mod = node_from_bytes(&mut a, &NFT_ROYALTY_TRANSFER_PUZZLE)?;
        let ownership_layer_mod = node_from_bytes(&mut a, &NFT_OWNERSHIP_LAYER_PUZZLE)?;
        let state_layer_mod = node_from_bytes(&mut a, &NFT_STATE_LAYER_PUZZLE)?;
        let singleton_mod = node_from_bytes(&mut a, &SINGLETON_PUZZLE)?;
        let p2_mod = node_from_bytes(&mut a, &STANDARD_PUZZLE)?;

        // Construct the p2 puzzle.
        let p2_puzzle_hash = self.state.write().await.unused_puzzle_hash().await?;

        let p2_args = StandardArgs {
            synthetic_key: self
                .state
                .read()
                .await
                .key_store
                .secret_key_of(&p2_puzzle_hash)
                .ok_or(anyhow::Error::msg("missing secret key for p2 spend"))?
                .to_public_key(),
        }
        .to_clvm(&mut a)?;
        let p2 = curry(&mut a, p2_mod, p2_args)?;

        // Collect spend information for each NFT mint.
        let mut coin_spends = Vec::new();
        let mut did_condition_list = Vec::new();
        let mut signatures = Vec::new();
        let mut nft_ids = Vec::new();

        // Prepare NFT mint spends.
        for (raw_index, nft_mint) in nft_mints.iter().enumerate() {
            let index = start_index + raw_index;

            // Create intermediate launcher to prevent launcher id collisions.
            let intermediate_args = NftIntermediateLauncherArgs {
                launcher_puzzle_hash: LAUNCHER_PUZZLE_HASH,
                mint_number: index,
                mint_total: total_nft_count,
            }
            .to_clvm(&mut a)?;
            let intermediate_puzzle = curry(&mut a, intermediate_launcher_mod, intermediate_args)?;
            let intermediate_puzzle_hash = tree_hash(&a, intermediate_puzzle);

            let intermediate_coin = Coin::new(
                did_info.coin_state.coin.coin_id().into(),
                intermediate_puzzle_hash.into(),
                0,
            );

            let intermediate_coin_id = intermediate_coin.coin_id();

            did_condition_list.push(Condition::CreateCoin {
                puzzle_hash: intermediate_puzzle_hash,
                amount: 0,
                memos: vec![],
            });

            // Spend intermediate launcher.
            let intermediate_solution = a.null();

            let intermediate_spend = CoinSpend::new(
                intermediate_coin.clone(),
                Program::from_clvm(&a, intermediate_puzzle)?,
                Program::from_clvm(&a, intermediate_solution)?,
            );

            coin_spends.push(intermediate_spend);

            // Assert intermediate launcher info in DID spend.
            let mut hasher = Sha256::new();
            hasher.update(int_to_bytes(index.into()));
            hasher.update(int_to_bytes(total_nft_count.into()));
            let announcement_message: [u8; 32] = hasher.finalize_fixed().into();

            let mut hasher = Sha256::new();
            hasher.update(intermediate_coin_id);
            hasher.update(announcement_message);
            let announcement_id: [u8; 32] = hasher.finalize_fixed().into();

            did_condition_list.push(Condition::AssertCoinAnnouncement { announcement_id });

            // Create the launcher coin.
            let launcher_coin = Coin::new(
                intermediate_coin_id.into(),
                LAUNCHER_PUZZLE_HASH.into(),
                nft_amount,
            );
            let launcher_id = launcher_coin.coin_id();

            nft_ids.push(launcher_id);

            did_condition_list.push(Condition::CreatePuzzleAnnouncement {
                message: launcher_id,
            });

            let nft_singleton_struct = SingletonStruct::from_launcher_id(launcher_id);

            // Curry the NFT ownership layer for the eve coin.
            let eve_transfer_program_args = NftRoyaltyTransferPuzzleArgs {
                singleton_struct: nft_singleton_struct.clone(),
                royalty_puzzle_hash: nft_mint.royalty_puzzle_hash,
                trade_price_percentage: nft_mint.royalty_percentage,
            }
            .to_clvm(&mut a)?;

            let eve_transfer_program =
                curry(&mut a, transfer_program_mod, eve_transfer_program_args)?;

            let eve_ownership_layer_args = NftOwnershipLayerArgs {
                mod_hash: NFT_OWNERSHIP_LAYER_PUZZLE_HASH,
                current_owner: None,
                transfer_program: LazyNode(eve_transfer_program),
                inner_puzzle: LazyNode(p2),
            }
            .to_clvm(&mut a)?;

            let eve_ownership_layer = curry(&mut a, ownership_layer_mod, eve_ownership_layer_args)?;

            // Curry the NFT state layer for the eve coin.
            let metadata = nft_mint.metadata.to_clvm(&mut a)?;

            let eve_state_layer_args = NftStateLayerArgs {
                mod_hash: NFT_STATE_LAYER_PUZZLE_HASH,
                metadata: LazyNode(metadata),
                metadata_updater_puzzle_hash: NFT_METADATA_UPDATER_PUZZLE_HASH,
                inner_puzzle: LazyNode(eve_ownership_layer),
            }
            .to_clvm(&mut a)?;

            let eve_state_layer = curry(&mut a, state_layer_mod, eve_state_layer_args)?;

            // Curry the singleton for the eve coin.
            let eve_singleton_args = SingletonArgs {
                singleton_struct: nft_singleton_struct,
                inner_puzzle: LazyNode(eve_state_layer),
            }
            .to_clvm(&mut a)?;

            let eve_singleton = curry(&mut a, singleton_mod, eve_singleton_args)?;
            let eve_puzzle_hash = tree_hash(&a, eve_singleton);

            // The DID spend will assert an announcement from the eve coin.
            let announcement_message_content =
                clvm_list!(eve_puzzle_hash, nft_amount, ()).to_clvm(&mut a)?;
            let announcement_message = tree_hash(&a, announcement_message_content);

            let mut hasher = Sha256::new();
            hasher.update(launcher_id);
            hasher.update(announcement_message);
            let announcement_id: [u8; 32] = hasher.finalize_fixed().into();

            did_condition_list.push(Condition::AssertCoinAnnouncement { announcement_id });

            // Spend the launcher coin.
            let launcher_solution = LauncherSolution {
                singleton_puzzle_hash: eve_puzzle_hash,
                amount: nft_amount,
                key_value_list: LazyNode(a.null()),
            }
            .to_clvm(&mut a)?;

            let launcher_spend = CoinSpend::new(
                launcher_coin.clone(),
                Program::parse(&mut Cursor::new(&LAUNCHER_PUZZLE))?,
                Program::from_clvm(&a, launcher_solution)?,
            );

            coin_spends.push(launcher_spend);

            // Create the eve coin info.
            let eve_coin = Coin::new(
                launcher_coin.coin_id().into(),
                eve_puzzle_hash.into(),
                nft_amount,
            );

            let eve_proof = EveProof {
                parent_coin_info: intermediate_coin.coin_id(),
                amount: nft_amount,
            };

            self.state.write().await.update_nft(NftInfo {
                launcher_id,
                puzzle_reveal: Program::from_clvm(&a, eve_singleton)?,
                p2_puzzle_hash,
                coin_state: CoinState::new(eve_coin, None, None),
                proof: Proof::Eve(eve_proof),
            })?;

            // Create eve coin spend.
            let eve_spend_conditions = vec![Condition::CreateCoin {
                puzzle_hash: nft_mint.target_puzzle_hash,
                amount: nft_amount as i64,
                memos: vec![nft_mint.target_puzzle_hash],
            }];

            let (eve_coin_spend, signature, announcement_message) = self
                .spend_nft(
                    &launcher_id,
                    NewOwner::DidInfo {
                        did_id: did_info.launcher_id,
                        did_inner_puzzle_hash: did_info.inner_puzzle_hash,
                    },
                    eve_spend_conditions,
                )
                .await?;

            coin_spends.push(eve_coin_spend);
            signatures.push(signature);

            // Assert eve puzzle announcement in funding spend.
            let mut hasher = Sha256::new();
            hasher.update(eve_puzzle_hash);
            hasher.update(announcement_message.unwrap());
            let announcement_id: [u8; 32] = hasher.finalize_fixed().into();

            did_condition_list.push(Condition::AssertPuzzleAnnouncement { announcement_id });
        }

        // Calculate change.
        let spent_amount = selected_coins
            .iter()
            .fold(0, |amount, coin| amount + coin.amount);
        let change_amount = spent_amount - required_amount;
        let change_puzzle_hash = self.state.write().await.unused_puzzle_hash().await?;

        // Calculate announcement message.
        let mut hasher = Sha256::new();
        selected_coins
            .iter()
            .for_each(|coin| hasher.update(coin.coin_id()));
        if change_amount > 0 {
            hasher.update(
                Coin::new(
                    funding_coin.coin_id().into(),
                    change_puzzle_hash.into(),
                    change_amount,
                )
                .coin_id(),
            );
        }

        let announcement_message: [u8; 32] = hasher.finalize_fixed().into();

        did_condition_list.push(Condition::CreateCoinAnnouncement {
            message: announcement_message,
        });

        // Calculate primary announcement id.
        let mut hasher = Sha256::new();
        hasher.update(funding_coin.coin_id());
        hasher.update(announcement_message);
        let primary_announcement_id: [u8; 32] = hasher.finalize_fixed().into();

        // Spend standard coins.
        for (index, coin) in selected_coins.iter().enumerate() {
            // Fetch the key pair.
            let secret_key = self
                .state
                .read()
                .await
                .key_store
                .secret_key_of((&coin.puzzle_hash).into())
                .ok_or(anyhow::Error::msg("missing secret key for fee coin spend"))?
                .clone();
            let public_key = secret_key.to_public_key();

            // Construct the p2 puzzle.
            let fee_p2_args = StandardArgs {
                synthetic_key: public_key,
            }
            .to_clvm(&mut a)?;
            let fee_p2 = curry(&mut a, p2_mod, fee_p2_args)?;

            // Calculate the conditions.
            let condition_list = if index == 0 {
                let mut condition_list = vec![];

                // Announce to other coins.
                if selected_coins.len() > 1 {
                    condition_list.push(Condition::CreateCoinAnnouncement {
                        message: announcement_message,
                    });
                }

                // Assert DID announcement.
                let mut hasher = Sha256::new();
                hasher.update(did_info.coin_state.coin.coin_id());
                hasher.update(announcement_message);
                let did_announcement_id: [u8; 32] = hasher.finalize_fixed().into();

                condition_list.push(Condition::AssertCoinAnnouncement {
                    announcement_id: did_announcement_id,
                });

                // Create change coin.
                if change_amount > 0 {
                    condition_list.push(Condition::CreateCoin {
                        puzzle_hash: change_puzzle_hash,
                        amount: change_amount as i64,
                        memos: vec![],
                    });
                }

                condition_list
            } else {
                vec![Condition::AssertCoinAnnouncement {
                    announcement_id: primary_announcement_id,
                }]
            };

            let conditions = clvm_quote!(condition_list).to_clvm(&mut a)?;
            let conditions_tree_hash = tree_hash(&a, conditions);
            let solution = StandardSolution::with_conditions(&mut a, conditions).to_clvm(&mut a)?;

            // Sign the spend.
            let signature = sign_agg_sig_me(
                &secret_key,
                &conditions_tree_hash,
                &coin.coin_id(),
                &self.peer.network.agg_sig_me_extra_data,
            );

            signatures.push(signature);

            // Create the coin spend.
            let coin_spend = CoinSpend::new(
                coin.clone(),
                Program::from_clvm(&a, fee_p2)?,
                Program::from_clvm(&a, solution)?,
            );

            coin_spends.push(coin_spend);
        }

        let (did_message_spend, did_signature) = self
            .spend_did(
                did_id,
                did_info.inner_puzzle_hash,
                did_info.p2_puzzle_hash,
                did_condition_list,
            )
            .await?;

        coin_spends.push(did_message_spend);
        signatures.push(did_signature);

        Ok((
            coin_spends,
            signatures
                .into_iter()
                .reduce(|aggregate, signature| aggregate.add(&signature))
                .unwrap(),
            nft_ids,
        ))
    }

    pub async fn spend_nft(
        &self,
        nft_id: &[u8; 32],
        new_owner: NewOwner,
        condition_list: Vec<Condition>,
    ) -> anyhow::Result<(CoinSpend, Signature, Option<Vec<u8>>)> {
        // Get NFT info.
        let nft_info = self
            .state
            .read()
            .await
            .get_nft_info(nft_id)
            .ok_or(anyhow::Error::msg("could not find NFT info"))?;

        // Initialize the allocator.
        let mut a = Allocator::new();

        // Construct the p2 solution.
        let conditions: NodePtr;
        let mut announcement_message = None;

        match new_owner {
            NewOwner::DidInfo {
                did_id,
                did_inner_puzzle_hash,
            } => {
                let new_owner_condition_args =
                    clvm_list!(did_id, (), did_inner_puzzle_hash).to_clvm(&mut a)?;
                let magic_condition = (-10, LazyNode(new_owner_condition_args)).to_clvm(&mut a)?;
                conditions =
                    clvm_quote!((LazyNode(magic_condition), condition_list)).to_clvm(&mut a)?;

                let mut message = vec![0xad, 0x4c];
                message.extend(tree_hash(&a, new_owner_condition_args));
                announcement_message = Some(message);
            }
            NewOwner::Reset => {
                let new_owner_condition_args = clvm_list!((), (), ()).to_clvm(&mut a)?;
                let magic_condition = (-10, LazyNode(new_owner_condition_args)).to_clvm(&mut a)?;
                conditions =
                    clvm_quote!((LazyNode(magic_condition), condition_list)).to_clvm(&mut a)?;

                let mut message = vec![0xad, 0x4c];
                message.extend(tree_hash(&a, new_owner_condition_args));
                announcement_message = Some(message);
            }
            NewOwner::Retain => conditions = clvm_quote!(condition_list).to_clvm(&mut a)?,
        }

        let conditions_tree_hash = tree_hash(&a, conditions);
        let p2_solution = StandardSolution::with_conditions(&mut a, conditions).to_clvm(&mut a)?;

        // Sign the spend.
        let signature = sign_agg_sig_me(
            self.state
                .read()
                .await
                .key_store
                .secret_key_of(&nft_info.p2_puzzle_hash)
                .ok_or(anyhow::Error::msg("missing secret key for NFT coin spend"))?,
            &conditions_tree_hash,
            &nft_info.coin_state.coin.coin_id(),
            &self.peer.network.agg_sig_me_extra_data,
        );

        // Construct the ownership layer solution.
        let ownership_layer_solution = NftOwnershipLayerSolution {
            inner_solution: LazyNode(p2_solution),
        }
        .to_clvm(&mut a)?;

        // Construct the state layer solution.
        let state_layer_solution = NftStateLayerSolution {
            inner_solution: LazyNode(ownership_layer_solution),
        }
        .to_clvm(&mut a)?;

        // Construct the singleton solution.
        let solution = SingletonSolution {
            proof: nft_info.proof.clone(),
            amount: nft_info.coin_state.coin.amount,
            inner_solution: LazyNode(state_layer_solution),
        }
        .to_clvm(&mut a)?;

        // Construct the coin spend.
        let coin_spend = CoinSpend::new(
            nft_info.coin_state.coin.clone(),
            nft_info.puzzle_reveal.clone(),
            Program::from_clvm(&a, solution)?,
        );

        Ok((coin_spend, signature, announcement_message))
    }

    pub async fn spend_did(
        &self,
        did_id: &[u8; 32],
        new_inner_puzzle_hash: [u8; 32],
        hint: [u8; 32],
        extra_condition_list: Vec<Condition>,
    ) -> anyhow::Result<(CoinSpend, Signature)> {
        // Get DID info.
        let did_info = self
            .state
            .read()
            .await
            .get_did_info(did_id)
            .ok_or(anyhow::Error::msg("could not find DID info"))?;

        // Initialize the allocator.
        let mut a = Allocator::new();

        // Spend standard puzzle.
        let mut condition_list = vec![Condition::CreateCoin {
            puzzle_hash: new_inner_puzzle_hash,
            amount: did_info.coin_state.coin.amount as i64,
            memos: vec![hint],
        }];
        condition_list.extend(extra_condition_list);
        let conditions = clvm_quote!(condition_list).to_clvm(&mut a)?;
        let conditions_tree_hash = tree_hash(&a, conditions);
        let p2_solution = StandardSolution::with_conditions(&mut a, conditions).to_clvm(&mut a)?;

        // Sign the spend.
        let signature = sign_agg_sig_me(
            self.state
                .read()
                .await
                .key_store
                .secret_key_of(&did_info.p2_puzzle_hash)
                .ok_or(anyhow::Error::msg("missing secret key for DID coin spend"))?,
            &conditions_tree_hash,
            &did_info.coin_state.coin.coin_id(),
            &self.peer.network.agg_sig_me_extra_data,
        );

        // Spend DID puzzle.
        let did_solution = DidSolution::InnerSpend(LazyNode(p2_solution)).to_clvm(&mut a)?;

        // Spend singleton.
        let solution = SingletonSolution {
            proof: did_info.proof.clone(),
            amount: did_info.coin_state.coin.amount,
            inner_solution: LazyNode(did_solution),
        }
        .to_clvm(&mut a)?;

        // Create the coin spend.
        let coin_spend = CoinSpend::new(
            did_info.coin_state.coin.clone(),
            did_info.puzzle_reveal.clone(),
            Program::from_clvm(&a, solution)?,
        );

        Ok((coin_spend, signature))
    }

    pub async fn join(&mut self) -> Result<(), JoinError> {
        if let Some(handle) = self.runner_handle.take() {
            handle.await?;
        }
        Ok(())
    }
}

impl Drop for Wallet {
    fn drop(&mut self) {
        if let Some(handle) = self.runner_handle.take() {
            handle.abort();
        }
    }
}
