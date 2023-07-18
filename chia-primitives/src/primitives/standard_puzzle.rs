use chia_bls::PublicKey;
use chia_protocol::Program;
use clvm_utils::{curry, curry_tree_hash, new_list, tree_hash_atom, uncurry, Allocate};
use clvmr::{allocator::NodePtr, reduction::EvalErr, serde::node_from_bytes, Allocator};

use crate::puzzles::{P2_DELEGATED_OR_HIDDEN, P2_DELEGATED_OR_HIDDEN_HASH};

#[derive(Debug, Clone)]
pub struct StandardPuzzle {
    synthetic_key: PublicKey,
}

impl Allocate for StandardPuzzle {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Option<Self> {
        let (program, args) = uncurry(a, node)?;
        let program = Program::from_clvm(a, program)?;
        if program.as_ref() != P2_DELEGATED_OR_HIDDEN || args.len() != 1 {
            return None;
        }
        Some(Self {
            synthetic_key: Allocate::from_clvm(a, args[0])?,
        })
    }
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr, EvalErr> {
        let node = node_from_bytes(a, &P2_DELEGATED_OR_HIDDEN)
            .map_err(|error| EvalErr(a.null(), error.to_string()))?;

        let synthetic_key = self.synthetic_key.to_clvm(a)?;

        curry(a, node, &[synthetic_key])
    }
}

pub fn alloc_standard_puzzle(a: &mut Allocator) -> std::io::Result<NodePtr> {
    node_from_bytes(a, &P2_DELEGATED_OR_HIDDEN)
}

pub fn standard_puzzle_hash(synthetic_key: PublicKey) -> [u8; 32] {
    let synthetic_key = tree_hash_atom(&synthetic_key.to_bytes());
    curry_tree_hash(&P2_DELEGATED_OR_HIDDEN_HASH, &[&synthetic_key])
}

pub fn curry_standard_puzzle(
    a: &mut Allocator,
    node: NodePtr,
    synthetic_key: PublicKey,
) -> Result<NodePtr, EvalErr> {
    let synthetic_key = a.new_atom(&synthetic_key.to_bytes())?;
    curry(a, node, &[synthetic_key])
}

pub fn solve_standard_puzzle(
    a: &mut Allocator,
    conditions: &[NodePtr],
) -> Result<NodePtr, EvalErr> {
    let condition_list = new_list(a, conditions)?;
    let delegated_puzzle = a.new_pair(a.one(), condition_list)?;
    let nil = a.null();
    new_list(a, &[nil, delegated_puzzle, nil])
}
