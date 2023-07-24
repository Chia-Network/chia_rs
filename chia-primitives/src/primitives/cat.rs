use chia_bls::PublicKey;
use chia_protocol::Coin;
use clvm_utils::{FromClvm, LazyNode, ToClvm};

use crate::LineageProof;

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct CatArgs {
    pub mod_hash: [u8; 32],
    pub tail_program_hash: [u8; 32],
    pub inner_puzzle: LazyNode,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct EverythingWithSignatureTailArgs {
    pub public_key: PublicKey,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(proper_list)]
pub struct CatSolution {
    pub inner_puzzle_solution: LazyNode,
    pub lineage_proof: Option<LineageProof>,
    pub prev_coin_id: [u8; 32],
    pub this_coin_info: Coin,
    pub next_coin_proof: CoinProof,
    pub prev_subtotal: i64,
    pub extra_delta: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(proper_list)]
pub struct CoinProof {
    pub parent_coin_info: [u8; 32],
    pub inner_puzzle_hash: [u8; 32],
    pub amount: u64,
}
