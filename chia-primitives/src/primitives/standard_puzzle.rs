use chia_bls::PublicKey;
use clvm_utils::{curry_tree_hash, tree_hash_atom, FromClvm, ToClvm};
use clvmr::{allocator::NodePtr, Allocator};

use crate::{condition::Condition, puzzles::P2_DELEGATED_OR_HIDDEN_HASH};

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct StandardPuzzle {
    pub synthetic_key: PublicKey,
}

pub fn standard_puzzle_hash(synthetic_key: PublicKey) -> [u8; 32] {
    let synthetic_key = tree_hash_atom(&synthetic_key.to_bytes());
    curry_tree_hash(&P2_DELEGATED_OR_HIDDEN_HASH, &[&synthetic_key])
}

pub fn standard_solution(
    a: &mut Allocator,
    conditions: &[Condition],
) -> clvm_utils::Result<NodePtr> {
    ToClvm::to_clvm(&((), ((1, conditions.to_vec()), ((), ()))), a)
}
