use chia_bls::PublicKey;
use clvm_utils::{clvm_quote, curry_tree_hash, match_quote, tree_hash_atom, FromClvm, ToClvm};

use crate::{condition::Condition, puzzles::P2_DELEGATED_OR_HIDDEN_HASH};

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct StandardPuzzle {
    pub synthetic_key: PublicKey,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(proper_list)]
pub struct StandardSolution<T, S> {
    pub original_public_key: Option<PublicKey>,
    pub delegated_puzzle: T,
    pub solution: S,
}

impl StandardSolution<match_quote!(Vec<Condition>), ()> {
    pub fn with_conditions(conditions: Vec<Condition>) -> Self {
        Self {
            original_public_key: None,
            delegated_puzzle: clvm_quote!(conditions),
            solution: (),
        }
    }
}

pub fn standard_puzzle_hash(synthetic_key: PublicKey) -> [u8; 32] {
    let synthetic_key = tree_hash_atom(&synthetic_key.to_bytes());
    curry_tree_hash(&P2_DELEGATED_OR_HIDDEN_HASH, &[&synthetic_key])
}
