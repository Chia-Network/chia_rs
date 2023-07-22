use chia_bls::PublicKey;
use clvm_utils::{curry_tree_hash, tree_hash_atom, FromClvm, LazyNode, ToClvm};
use clvmr::{allocator::NodePtr, Allocator};

use crate::puzzles::P2_DELEGATED_OR_HIDDEN_HASH;

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct StandardPuzzle {
    pub synthetic_key: PublicKey,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(proper_list)]
pub struct StandardSolution {
    pub original_public_key: Option<PublicKey>,
    pub delegated_puzzle: LazyNode,
    pub solution: LazyNode,
}

impl StandardSolution {
    pub fn with_conditions(a: &mut Allocator, conditions: NodePtr) -> Self {
        Self {
            original_public_key: None,
            delegated_puzzle: LazyNode(conditions),
            solution: LazyNode(a.null()),
        }
    }
}

pub fn standard_puzzle_hash(synthetic_key: &PublicKey) -> [u8; 32] {
    let synthetic_key = tree_hash_atom(&synthetic_key.to_bytes());
    curry_tree_hash(&P2_DELEGATED_OR_HIDDEN_HASH, &[&synthetic_key])
}
