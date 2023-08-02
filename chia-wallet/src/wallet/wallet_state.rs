use std::sync::Arc;

use chia_client::Peer;
use chia_primitives::{
    puzzles::{
        CAT_PUZZLE_HASH, DID_INNER_PUZZLE_HASH, NFT_OWNERSHIP_LAYER_PUZZLE_HASH,
        NFT_STATE_LAYER_PUZZLE, NFT_STATE_LAYER_PUZZLE_HASH, SINGLETON_PUZZLE,
        SINGLETON_PUZZLE_HASH, STANDARD_PUZZLE,
    },
    CatArgs, Condition, DidArgs, LineageProof, NftMetadata, NftOwnershipLayerArgs,
    NftOwnershipLayerSolution, NftStateLayerArgs, NftStateLayerSolution, Proof, SingletonArgs,
    SingletonSolution, SingletonStruct, StandardArgs,
};
use chia_protocol::{
    Coin, CoinState, Program, RegisterForCoinUpdates, RegisterForPhUpdates, RequestPuzzleSolution,
    RespondPuzzleSolution, RespondToCoinUpdates, RespondToPhUpdates,
};
use clvm_utils::{
    curry, match_list, match_tuple, tree_hash, uncurry, FromClvm, LazyNode, MatchByte, ToClvm,
};
use clvmr::{allocator::NodePtr, run_program, serde::node_from_bytes, Allocator, ChiaDialect};
use tokio::sync::broadcast::Sender;

use crate::utils::{select_coins, update_state};

use super::{CatCoin, CatInfo, DidInfo, KeyStore, NftInfo, WalletEvent};

pub struct WalletState {
    pub peer: Arc<Peer>,
    pub key_store: KeyStore,
    event_sender: Sender<WalletEvent>,
    standard_coins: Vec<CoinState>,
    did_coins: Vec<DidInfo>,
    nft_coins: Vec<NftInfo>,
    cat_coins: Vec<CatInfo>,
}

impl WalletState {
    pub fn new(peer: Arc<Peer>, key_store: KeyStore, event_sender: Sender<WalletEvent>) -> Self {
        Self {
            peer,
            key_store,
            event_sender,
            standard_coins: Vec::new(),
            did_coins: Vec::new(),
            nft_coins: Vec::new(),
            cat_coins: Vec::new(),
        }
    }

    /// Fetches the DID info for a given launcher id.
    pub fn get_did_info(&self, did_id: &[u8; 32]) -> Option<DidInfo> {
        self.did_coins
            .iter()
            .find(|item| &item.launcher_id == did_id)
            .cloned()
    }

    /// Fetches the NFT info for a given launcher id.
    pub fn get_nft_info(&self, nft_id: &[u8; 32]) -> Option<NftInfo> {
        self.nft_coins
            .iter()
            .find(|item| &item.launcher_id == nft_id)
            .cloned()
    }

    /// Calculates the next unused puzzle hash.
    pub async fn unused_puzzle_hash(&mut self) -> anyhow::Result<[u8; 32]> {
        let mut puzzle_hashes = self.key_store.puzzle_hashes();
        loop {
            for puzzle_hash in puzzle_hashes.iter() {
                let has_coins = self
                    .standard_coins
                    .iter()
                    .any(|item| item.coin.puzzle_hash == puzzle_hash);

                if !has_coins {
                    return Ok(*puzzle_hash);
                }
            }
            puzzle_hashes = self.generate_puzzle_hashes().await?;
        }
    }

    /// Selects standard p2 coins to spend.
    pub fn select_standard_coins(&self, amount: u64) -> Vec<Coin> {
        let spendable_coins = self.spendable_standard_coins();
        select_coins(spendable_coins, amount)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Selects CAT coins to spend.
    pub fn select_cat_coins(&self, asset_id: &[u8; 32], amount: u64) -> Vec<CatCoin> {
        let spendable_coins: Vec<CatCoin> = self
            .spendable_cat_coins(asset_id)
            .into_iter()
            .cloned()
            .collect();

        select_coins(
            spendable_coins
                .iter()
                .map(|item| &item.coin_state.coin)
                .collect(),
            amount,
        )
        .into_iter()
        .map(|selected| {
            spendable_coins
                .iter()
                .find(|item| selected == &item.coin_state.coin)
                .unwrap()
        })
        .cloned()
        .collect()
    }

    /// Fetches a list of spendable standard coins.
    fn spendable_standard_coins(&self) -> Vec<&Coin> {
        self.standard_coins
            .iter()
            .filter(|item| item.created_height.is_some() && item.spent_height.is_none())
            .map(|item| &item.coin)
            .collect()
    }

    /// Fetches a list of spendable CAT coins.
    fn spendable_cat_coins(&self, asset_id: &[u8; 32]) -> Vec<&CatCoin> {
        let Some(cat_info) = self
            .cat_coins
            .iter()
            .find(|item| &item.asset_id == asset_id) else {
                return Vec::new();
            };

        cat_info
            .coins
            .iter()
            .filter(|item| {
                item.coin_state.created_height.is_some() && item.coin_state.spent_height.is_none()
            })
            .collect()
    }

    /// Generates new wallet addresses and registers for updates.
    async fn generate_puzzle_hashes(&mut self) -> anyhow::Result<Vec<[u8; 32]>> {
        let puzzle_hashes: Vec<[u8; 32]> = (0..100).map(|_| self.key_store.derive_next()).collect();

        let response: RespondToPhUpdates = self
            .peer
            .request(RegisterForPhUpdates::new(
                puzzle_hashes
                    .iter()
                    .map(|puzzle_hash| puzzle_hash.into())
                    .collect(),
                0,
            ))
            .await?;

        self.update_unknown_coins(response.coin_states).await;

        Ok(puzzle_hashes)
    }

    /// Handles coin state updates.
    pub async fn update_unknown_coins(&mut self, updates: Vec<CoinState>) {
        for update in updates {
            let puzzle_hash: &[u8; 32] = (&update.coin.puzzle_hash).into();
            if self.key_store.contains_puzzle(puzzle_hash) {
                update_state(&mut self.standard_coins, update);
            } else {
                self.handle_hinted_coin(update).await.ok();
            }
        }
    }

    /// Handles pending DID updates.
    pub fn update_did(&mut self, did_info: DidInfo) -> anyhow::Result<()> {
        let did_id = did_info.launcher_id;
        let is_confirmed = did_info.coin_state.created_height.is_some();

        match self
            .did_coins
            .iter_mut()
            .find(|item| item.launcher_id == did_info.launcher_id)
        {
            Some(existing) => *existing = did_info,
            None => self.did_coins.push(did_info),
        }

        if is_confirmed {
            self.event_sender
                .send(WalletEvent::DidConfirmed { did_id })?;
        }

        Ok(())
    }

    /// Handles pending NFT updates.
    pub fn update_nft(&mut self, nft_info: NftInfo) -> anyhow::Result<()> {
        let nft_id = nft_info.launcher_id;
        let is_confirmed = nft_info.coin_state.created_height.is_some();

        match self
            .nft_coins
            .iter_mut()
            .find(|item| item.launcher_id == nft_info.launcher_id)
        {
            Some(existing) => *existing = nft_info,
            None => self.nft_coins.push(nft_info),
        }

        if is_confirmed {
            self.event_sender
                .send(WalletEvent::NftConfirmed { nft_id })?;
        }

        Ok(())
    }

    /// Handles pending CAT updates.
    pub fn update_cat(&mut self, asset_id: [u8; 32], update: CatCoin) -> anyhow::Result<()> {
        let is_confirmed = update.coin_state.created_height.is_some();

        match self
            .cat_coins
            .iter_mut()
            .find(|item| item.asset_id == asset_id)
        {
            Some(existing) => match existing
                .coins
                .iter_mut()
                .find(|item| item.coin_state.coin == update.coin_state.coin)
            {
                Some(value) => *value = update,
                None => existing.coins.push(update),
            },
            None => {
                self.cat_coins.push(CatInfo {
                    asset_id,
                    tail: None,
                    coins: vec![update],
                });

                if is_confirmed {
                    self.event_sender
                        .send(WalletEvent::CatDiscovered { asset_id })?;
                }
            }
        }

        Ok(())
    }

    fn discover_did(
        &mut self,
        a: &Allocator,
        parent_coin_state: CoinState,
        coin_state: CoinState,
        parent_puzzle: Program,
        launcher_id: [u8; 32],
        inner_puzzle_hash: [u8; 32],
        did_args: DidArgs,
    ) -> anyhow::Result<()> {
        let lineage_parent: &[u8; 32] = (&parent_coin_state.coin.parent_coin_info).into();

        let lineage_proof = LineageProof {
            parent_coin_info: *lineage_parent,
            inner_puzzle_hash,
            amount: parent_coin_state.coin.amount,
        };

        self.update_did(DidInfo {
            launcher_id,
            coin_state,
            puzzle_reveal: parent_puzzle,
            inner_puzzle_hash,
            p2_puzzle_hash: tree_hash(a, did_args.inner_puzzle.0),
            proof: Proof::Lineage(lineage_proof),
        })?;

        Ok(())
    }

    fn discover_nft(
        &mut self,
        a: &mut Allocator,
        dialect: &ChiaDialect,
        parent_coin_state: CoinState,
        coin_state: CoinState,
        parent_solution: NodePtr,
        launcher_id: [u8; 32],
        inner_puzzle_hash: [u8; 32],
        state_layer_args: NftStateLayerArgs,
    ) -> anyhow::Result<()> {
        let singleton_solution = SingletonSolution::from_clvm(a, parent_solution)?;
        let state_solution =
            NftStateLayerSolution::from_clvm(a, singleton_solution.inner_solution.0)?;
        let ownership_solution =
            NftOwnershipLayerSolution::from_clvm(a, state_solution.inner_solution.0)?;

        let (ownership_layer_mod, ownership_layer_args) =
            uncurry(a, state_layer_args.inner_puzzle.0)?;

        if tree_hash(a, ownership_layer_mod) != NFT_OWNERSHIP_LAYER_PUZZLE_HASH {
            return Err(anyhow::Error::msg("invalid NFT1 ownership puzzle"));
        }

        let ownership_layer_args = NftOwnershipLayerArgs::from_clvm(a, ownership_layer_args)?;

        let conditions = run_program(
            a,
            dialect,
            ownership_layer_args.inner_puzzle.0,
            ownership_solution.inner_solution.0,
            u64::MAX,
        )
        .map_err(clvm_utils::Error::Allocator)?;

        let mut metadata = NftMetadata::from_clvm(a, state_layer_args.metadata.0)?;
        let mut p2_puzzle_hash = None;
        let mut current_owner = ownership_layer_args.current_owner;

        for LazyNode(condition) in Vec::from_clvm(a, conditions.1)? {
            if let Ok(Condition::CreateCoin {
                amount: 1, memos, ..
            }) = Condition::from_clvm(a, condition)
            {
                if p2_puzzle_hash == None && !memos.is_empty() {
                    p2_puzzle_hash = Some(*memos.first().unwrap());
                }
            } else if let Ok(result) =
                <match_list!(MatchByte<235>, LazyNode, (String, String))>::from_clvm(a, condition)
            {
                let (key, uri) = result.1 .1 .0;

                match key.as_str() {
                    "u" => metadata.data_uris.insert(0, uri),
                    "mu" => metadata.metadata_uris.insert(0, uri),
                    "lu" => metadata.license_uris.insert(0, uri),
                    _ => {}
                }
            } else if let Ok(result) =
                <match_tuple!(MatchByte<246>, Option<[u8; 32]>, LazyNode)>::from_clvm(a, condition)
            {
                current_owner = result.1 .0;
            }
        }

        let metadata = metadata.to_clvm(a)?;

        let p2_puzzle_hash = p2_puzzle_hash.ok_or(anyhow::Error::msg("missing hint"))?;

        let public_key = self
            .key_store
            .secret_key_of(&p2_puzzle_hash)
            .ok_or(anyhow::Error::msg(
                "inner p2 puzzle doesn't belong to wallet",
            ))?
            .to_public_key();

        let p2_mod = node_from_bytes(a, &STANDARD_PUZZLE)?;
        let state_layer_mod = node_from_bytes(a, &NFT_STATE_LAYER_PUZZLE)?;
        let singleton_mod = node_from_bytes(a, &SINGLETON_PUZZLE)?;

        let p2_args = StandardArgs {
            synthetic_key: public_key,
        }
        .to_clvm(a)?;

        let p2 = curry(a, p2_mod, p2_args)?;

        let new_ownership_args = NftOwnershipLayerArgs {
            mod_hash: NFT_OWNERSHIP_LAYER_PUZZLE_HASH,
            current_owner,
            transfer_program: ownership_layer_args.transfer_program,
            inner_puzzle: LazyNode(p2),
        }
        .to_clvm(a)?;

        let new_ownership = curry(a, ownership_layer_mod, new_ownership_args)?;

        let new_state_args = NftStateLayerArgs {
            mod_hash: NFT_STATE_LAYER_PUZZLE_HASH,
            metadata: LazyNode(metadata),
            metadata_updater_puzzle_hash: state_layer_args.metadata_updater_puzzle_hash,
            inner_puzzle: LazyNode(new_ownership),
        }
        .to_clvm(a)?;

        let new_state = curry(a, state_layer_mod, new_state_args)?;

        let singleton_args = SingletonArgs {
            singleton_struct: SingletonStruct::from_launcher_id(launcher_id),
            inner_puzzle: LazyNode(new_state),
        }
        .to_clvm(a)?;

        let singleton = curry(a, singleton_mod, singleton_args)?;

        let lineage_parent: &[u8; 32] = (&parent_coin_state.coin.parent_coin_info).into();
        let lineage_proof = LineageProof {
            parent_coin_info: *lineage_parent,
            inner_puzzle_hash,
            amount: parent_coin_state.coin.amount,
        };

        self.update_nft(NftInfo {
            launcher_id,
            coin_state,
            puzzle_reveal: Program::from_clvm(a, singleton)?,
            p2_puzzle_hash,
            proof: Proof::Lineage(lineage_proof),
        })?;

        Ok(())
    }

    fn discover_cat(
        &mut self,
        a: &mut Allocator,
        dialect: &ChiaDialect,
        parent_coin_state: CoinState,
        coin_state: CoinState,
        parent_puzzle: NodePtr,
        parent_solution: NodePtr,
        cat_args: CatArgs,
    ) -> anyhow::Result<()> {
        let conditions = run_program(a, dialect, parent_puzzle, parent_solution, u64::MAX)
            .map_err(clvm_utils::Error::Allocator)?;

        let conditions = Vec::<clvm_utils::Result<Condition>>::from_clvm(a, conditions.1)?;

        let p2_puzzle_hash = conditions
            .iter()
            .find_map(|condition| {
                if let Ok(Condition::CreateCoin {
                    puzzle_hash, memos, ..
                }) = condition
                {
                    if puzzle_hash == coin_state.coin.puzzle_hash && !memos.is_empty() {
                        Some(*memos.first().unwrap())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .ok_or(anyhow::Error::msg("missing hint"))?;

        let lineage_parent: &[u8; 32] = (&parent_coin_state.coin.parent_coin_info).into();

        let lineage_proof = LineageProof {
            parent_coin_info: *lineage_parent,
            inner_puzzle_hash: tree_hash(a, cat_args.inner_puzzle.0),
            amount: parent_coin_state.coin.amount,
        };

        self.update_cat(
            cat_args.tail_program_hash,
            CatCoin {
                coin_state,
                lineage_proof,
                p2_puzzle_hash,
            },
        )?;

        Ok(())
    }

    /// Handles hinted coin discovery.
    async fn handle_hinted_coin(&mut self, coin_state: CoinState) -> anyhow::Result<()> {
        // Ignore spent coins.
        if coin_state.spent_height.is_some() {
            return Ok(());
        }

        // Request parent coin state.
        let mut response: RespondToCoinUpdates = self
            .peer
            .request(RegisterForCoinUpdates::new(
                vec![coin_state.coin.parent_coin_info],
                0,
            ))
            .await?;

        if response.coin_states.is_empty() {
            return Ok(());
        }

        let parent_coin_state = response.coin_states.remove(0);

        // Request parent coin spend.
        let response: RespondPuzzleSolution = self
            .peer
            .request(RequestPuzzleSolution::new(
                coin_state.coin.parent_coin_info,
                coin_state.created_height.unwrap_or_default(),
            ))
            .await?;

        let parent_spend = response.response;

        // Initialize the allocator.
        let mut a = Allocator::new();
        let dialect = ChiaDialect::new(0);

        let parent_puzzle = parent_spend.puzzle.to_clvm(&mut a)?;
        let parent_solution = parent_spend.solution.to_clvm(&mut a)?;

        let (uncurried_parent, parent_args) = uncurry(&a, parent_puzzle)?;

        match tree_hash(&a, uncurried_parent) {
            SINGLETON_PUZZLE_HASH => {
                let singleton_args = SingletonArgs::from_clvm(&a, parent_args)?;
                let (uncurried_inner, inner_args) = uncurry(&a, singleton_args.inner_puzzle.0)?;

                let launcher_id = singleton_args.singleton_struct.launcher_id;
                let inner_puzzle_hash = tree_hash(&a, singleton_args.inner_puzzle.0);

                match tree_hash(&a, uncurried_inner) {
                    DID_INNER_PUZZLE_HASH => {
                        let did_args = DidArgs::from_clvm(&a, inner_args)?;

                        self.discover_did(
                            &a,
                            parent_coin_state,
                            coin_state,
                            parent_spend.puzzle,
                            launcher_id,
                            inner_puzzle_hash,
                            did_args,
                        )?;
                    }
                    NFT_STATE_LAYER_PUZZLE_HASH => {
                        let state_layer_args = NftStateLayerArgs::from_clvm(&a, inner_args)?;

                        self.discover_nft(
                            &mut a,
                            &dialect,
                            parent_coin_state,
                            coin_state,
                            parent_solution,
                            launcher_id,
                            inner_puzzle_hash,
                            state_layer_args,
                        )?;
                    }
                    _ => {}
                }
            }
            CAT_PUZZLE_HASH => {
                let cat_args = CatArgs::from_clvm(&a, parent_args)?;

                self.discover_cat(
                    &mut a,
                    &dialect,
                    parent_coin_state,
                    coin_state,
                    parent_puzzle,
                    parent_solution,
                    cat_args,
                )?;
            }
            _ => {}
        }

        Ok(())
    }
}
