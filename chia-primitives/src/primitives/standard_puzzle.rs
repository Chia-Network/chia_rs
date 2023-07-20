use chia_bls::PublicKey;
use chia_protocol::Program;
use clvm_utils::{curry, curry_tree_hash, tree_hash_atom, uncurry, Allocate, Error};
use clvmr::{allocator::NodePtr, serde::node_from_bytes, Allocator};

use crate::{
    condition::Condition,
    puzzles::{P2_DELEGATED_OR_HIDDEN, P2_DELEGATED_OR_HIDDEN_HASH},
};

#[derive(Debug, Clone)]
pub struct StandardPuzzle {
    pub synthetic_key: PublicKey,
}

impl Allocate for StandardPuzzle {
    fn from_clvm(a: &Allocator, node: NodePtr) -> clvm_utils::Result<Self> {
        let (program, args) = uncurry(a, node)
            .ok_or_else(|| Error::Reason("could not uncurry program".to_string()))?;
        let program = Program::from_clvm(a, program)?;
        if program.as_ref() != P2_DELEGATED_OR_HIDDEN || args.len() != 1 {
            return Err(Error::Reason(
                "uncurried program is not standard puzzle".to_string(),
            ));
        }
        Ok(Self {
            synthetic_key: Allocate::from_clvm(a, args[0])?,
        })
    }
    fn to_clvm(&self, a: &mut Allocator) -> clvm_utils::Result<NodePtr> {
        let node = node_from_bytes(a, &P2_DELEGATED_OR_HIDDEN)?;
        let synthetic_key = self.synthetic_key.to_clvm(a)?;
        curry(a, node, &[synthetic_key]).map_err(Error::Eval)
    }
}

pub fn standard_puzzle_hash(synthetic_key: PublicKey) -> [u8; 32] {
    let synthetic_key = tree_hash_atom(&synthetic_key.to_bytes());
    curry_tree_hash(&P2_DELEGATED_OR_HIDDEN_HASH, &[&synthetic_key])
}

pub fn standard_solution(
    a: &mut Allocator,
    conditions: &[Condition],
) -> clvm_utils::Result<NodePtr> {
    Allocate::to_clvm(&((), ((1, conditions.to_vec()), ((), ()))), a)
}
