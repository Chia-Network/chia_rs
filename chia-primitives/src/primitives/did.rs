use chia_protocol::Program;
use clvm_utils::{curry, new_list, uncurry, Allocate, Error};
use clvmr::{allocator::NodePtr, reduction::EvalErr, serde::node_from_bytes, Allocator};

use crate::{puzzles::DID, singleton::SingletonStruct};

#[derive(Debug, Clone)]
pub struct Did<T = Program, M = Program>
where
    T: Allocate,
    M: Allocate,
{
    pub inner_puzzle: T,
    pub recovery_did_list_hash: [u8; 32],
    pub num_verifications_required: u64,
    pub singleton_struct: SingletonStruct,
    pub metadata: M,
}

impl<T, M> Allocate for Did<T, M>
where
    T: Allocate,
    M: Allocate,
{
    fn from_clvm(a: &Allocator, node: NodePtr) -> clvm_utils::Result<Self> {
        let (program, args) = uncurry(a, node)
            .ok_or_else(|| Error::Reason("could not uncurry program".to_string()))?;
        let program = Program::from_clvm(a, program)?;
        if program.as_ref() != DID || args.len() != 5 {
            return Err(Error::Reason("uncurried program is not did".to_string()));
        }
        Ok(Self {
            inner_puzzle: Allocate::from_clvm(a, args[0])?,
            recovery_did_list_hash: Allocate::from_clvm(a, args[1])?,
            num_verifications_required: Allocate::from_clvm(a, args[2])?,
            singleton_struct: Allocate::from_clvm(a, args[3])?,
            metadata: Allocate::from_clvm(a, args[4])?,
        })
    }
    fn to_clvm(&self, a: &mut Allocator) -> clvm_utils::Result<NodePtr> {
        let node = node_from_bytes(a, &DID)?;

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
        .map_err(Error::Eval)
    }
}

pub fn solve_did(a: &mut Allocator, inner_solution: NodePtr) -> Result<NodePtr, EvalErr> {
    let mode = a.one();
    new_list(a, &[mode, inner_solution])
}
