use clvm_utils::{FromClvm, LazyNode, ToClvm};

use crate::singleton::SingletonStruct;

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct NftState {
    pub mod_hash: [u8; 32],
    pub metadata: LazyNode,
    pub metadata_updater_puzzle_hash: [u8; 32],
    pub inner_puzzle: LazyNode,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(proper_list)]
pub struct NftStateSolution {
    pub inner_solution: LazyNode,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct NftOwnership {
    pub mod_hash: [u8; 32],
    pub current_owner: Option<[u8; 32]>,
    pub transfer_program: LazyNode,
    pub inner_puzzle: LazyNode,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(proper_list)]
pub struct NftOwnershipSolution {
    pub inner_solution: LazyNode,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct RoyaltyTransferProgram {
    pub singleton_struct: SingletonStruct,
    pub royalty_puzzle_hash: [u8; 32],
    pub trade_price_percentage: u16,
}
