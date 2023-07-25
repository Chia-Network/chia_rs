use chia_primitives::Proof;
use chia_protocol::{CoinState, Program};

#[derive(Debug, Clone)]
pub struct NftInfo {
    pub launcher_id: [u8; 32],
    pub coin_state: CoinState,
    pub puzzle_reveal: Program,
    pub p2_puzzle_hash: [u8; 32],
    pub proof: Proof,
}
