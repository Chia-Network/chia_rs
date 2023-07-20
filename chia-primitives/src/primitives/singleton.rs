use clvm_utils::{FromClvm, LazyNode, ToClvm};

use crate::Proof;

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct Singleton {
    pub singleton_struct: SingletonStruct,
    pub inner_puzzle: LazyNode,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(tuple)]
pub struct SingletonStruct {
    pub mod_hash: [u8; 32],
    pub launcher_id: [u8; 32],
    pub launcher_puzzle_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(proper_list)]
pub struct SingletonSolution {
    pub proof: Proof,
    pub amount: u64,
    pub inner_solution: LazyNode,
}
