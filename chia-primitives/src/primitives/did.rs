use chia_protocol::Program;
use clvm_utils::{curry, new_list, uncurry, Allocate};
use clvmr::{allocator::NodePtr, reduction::EvalErr, serde::node_from_bytes, Allocator};

use crate::{puzzles::DID, singleton::SingletonStruct};

#[derive(Debug, Clone)]
pub struct Did<T = Program, M = Program>
where
    T: Allocate,
    M: Allocate,
{
    inner_puzzle: T,
    recovery_did_list_hash: [u8; 32],
    num_verifications_required: u64,
    singleton_struct: SingletonStruct,
    metadata: M,
}

impl<T, M> Allocate for Did<T, M>
where
    T: Allocate,
    M: Allocate,
{
    fn from_clvm(a: &Allocator, node: NodePtr) -> Option<Self> {
        let (program, args) = uncurry(a, node)?;
        let program = Program::from_clvm(a, program)?;
        if program.as_ref() != DID || args.len() != 5 {
            return None;
        }
        Some(Self {
            inner_puzzle: Allocate::from_clvm(a, args[0])?,
            recovery_did_list_hash: Allocate::from_clvm(a, args[1])?,
            num_verifications_required: Allocate::from_clvm(a, args[2])?,
            singleton_struct: Allocate::from_clvm(a, args[3])?,
            metadata: Allocate::from_clvm(a, args[4])?,
        })
    }
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr, EvalErr> {
        let node =
            node_from_bytes(a, &DID).map_err(|error| EvalErr(a.null(), error.to_string()))?;

        let inner_puzzle = self.inner_puzzle.to_clvm(a)?;
        let recovery_did_list_hash = self.recovery_did_list_hash.to_clvm(a)?;
        let num_verifications_required = self.num_verifications_required.to_clvm(a)?;
        let singleton_struct = self.singleton_struct.to_clvm(a)?;
        let metadata = self.metadata.to_clvm(a)?;

        curry(
            a,
            node,
            &[
                inner_puzzle,
                recovery_did_list_hash,
                num_verifications_required,
                singleton_struct,
                metadata,
            ],
        )
    }
}

// pub fn curry_did(a: &mut Allocator, node: NodePtr, inner_puzzle: NodePtr, recovery)

pub fn solve_did(a: &mut Allocator, inner_solution: NodePtr) -> Result<NodePtr, EvalErr> {
    let mode = a.one();
    new_list(a, &[mode, inner_solution])
}
