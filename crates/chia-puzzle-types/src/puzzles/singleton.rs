use chia_protocol::Bytes32;
use chia_puzzles::{SINGLETON_LAUNCHER_HASH, SINGLETON_TOP_LAYER_V1_1_HASH};
use clvm_traits::{FromClvm, ToClvm};
use clvm_utils::{CurriedProgram, ToTreeHash, TreeHash};

use crate::Proof;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(curry)]
pub struct SingletonArgs<I> {
    pub singleton_struct: SingletonStruct,
    pub inner_puzzle: I,
}

impl<I> SingletonArgs<I> {
    pub fn new(launcher_id: Bytes32, inner_puzzle: I) -> Self {
        Self {
            singleton_struct: SingletonStruct::new(launcher_id),
            inner_puzzle,
        }
    }
}

impl SingletonArgs<TreeHash> {
    pub fn curry_tree_hash(launcher_id: Bytes32, inner_puzzle: TreeHash) -> TreeHash {
        CurriedProgram {
            program: TreeHash::new(SINGLETON_TOP_LAYER_V1_1_HASH),
            args: SingletonArgs::new(launcher_id, inner_puzzle),
        }
        .tree_hash()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct SingletonStruct {
    pub mod_hash: Bytes32,
    pub launcher_id: Bytes32,
    #[clvm(rest)]
    pub launcher_puzzle_hash: Bytes32,
}

impl SingletonStruct {
    pub fn new(launcher_id: Bytes32) -> Self {
        Self {
            mod_hash: SINGLETON_TOP_LAYER_V1_1_HASH.into(),
            launcher_id,
            launcher_puzzle_hash: SINGLETON_LAUNCHER_HASH.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct SingletonSolution<I> {
    pub lineage_proof: Proof,
    pub amount: u64,
    pub inner_solution: I,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct LauncherSolution<T> {
    pub singleton_puzzle_hash: Bytes32,
    pub amount: u64,
    pub key_value_list: T,
}
