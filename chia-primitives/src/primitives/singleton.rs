use clvm_utils::{FromClvm, LazyNode, ToClvm};

use crate::{
    puzzles::{LAUNCHER_PUZZLE_HASH, SINGLETON_PUZZLE_HASH},
    Proof,
};

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct SingletonArgs {
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

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(proper_list)]
pub struct LauncherSolution {
    pub singleton_puzzle_hash: [u8; 32],
    pub amount: u64,
    pub key_value_list: LazyNode,
}

impl SingletonStruct {
    pub fn from_launcher_id(launcher_id: [u8; 32]) -> Self {
        Self {
            mod_hash: SINGLETON_PUZZLE_HASH,
            launcher_id,
            launcher_puzzle_hash: LAUNCHER_PUZZLE_HASH,
        }
    }
}
