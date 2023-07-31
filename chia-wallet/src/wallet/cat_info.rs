use chia_primitives::{Condition, LineageProof};
use chia_protocol::{CoinState, Program};

#[derive(Debug)]
pub struct CatInfo {
    pub asset_id: [u8; 32],
    pub tail: Option<Program>,
    pub coins: Vec<CatCoin>,
}

#[derive(Debug, Clone)]
pub struct CatCoin {
    pub coin_state: CoinState,
    pub lineage_proof: LineageProof,
    pub p2_puzzle_hash: [u8; 32],
}

#[derive(Debug)]
pub struct CatSpend {
    pub cat_coin: CatCoin,
    pub conditions: Vec<Condition>,
    pub extra_delta: i64,
}
