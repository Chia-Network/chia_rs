use chia_protocol::Program;
use clvm_utils::{curry, new_list, uncurry, Allocate, Error};
use clvmr::{allocator::NodePtr, reduction::EvalErr, serde::node_from_bytes, Allocator};

use crate::{
    puzzles::{SINGLETON_LAUNCHER_HASH, SINGLETON_TOP_LAYER, SINGLETON_TOP_LAYER_HASH},
    Proof,
};

#[derive(Debug, Clone)]
pub struct Singleton<T = Program>
where
    T: Allocate,
{
    pub singleton_struct: SingletonStruct,
    pub inner_puzzle: T,
}

impl<T> Allocate for Singleton<T>
where
    T: Allocate,
{
    fn from_clvm(a: &Allocator, node: NodePtr) -> clvm_utils::Result<Self> {
        let (program, args) = uncurry(a, node)
            .ok_or_else(|| Error::Reason("could not uncurry program".to_string()))?;
        let program = Program::from_clvm(a, program)?;
        if program.as_ref() != SINGLETON_TOP_LAYER || args.len() != 2 {
            return Err(Error::Reason(
                "uncurried program is not singleton top layer".to_string(),
            ));
        }
        Ok(Self {
            singleton_struct: Allocate::from_clvm(a, args[0])?,
            inner_puzzle: Allocate::from_clvm(a, args[1])?,
        })
    }
    fn to_clvm(&self, a: &mut Allocator) -> clvm_utils::Result<NodePtr> {
        let node = node_from_bytes(a, &SINGLETON_TOP_LAYER)?;

        let singleton_struct = self.singleton_struct.to_clvm(a)?;
        let inner_puzzle = self.inner_puzzle.to_clvm(a)?;

        curry(a, node, &[singleton_struct, inner_puzzle]).map_err(Error::Eval)
    }
}

#[derive(Debug, Clone)]
pub struct SingletonStruct {
    pub mod_hash: [u8; 32],
    pub launcher_id: [u8; 32],
    pub launcher_puzzle_hash: [u8; 32],
}

impl Allocate for SingletonStruct {
    fn from_clvm(a: &Allocator, node: NodePtr) -> clvm_utils::Result<Self> {
        let value = <([u8; 32], ([u8; 32], [u8; 32]))>::from_clvm(a, node)?;
        Ok(Self {
            mod_hash: value.0,
            launcher_id: value.1 .0,
            launcher_puzzle_hash: value.1 .1,
        })
    }
    fn to_clvm(&self, a: &mut Allocator) -> clvm_utils::Result<NodePtr> {
        (self.mod_hash, (self.launcher_id, self.launcher_puzzle_hash)).to_clvm(a)
    }
}

pub fn alloc_singleton(a: &mut Allocator) -> std::io::Result<NodePtr> {
    node_from_bytes(a, &SINGLETON_TOP_LAYER)
}

pub fn curry_singleton(
    a: &mut Allocator,
    node: NodePtr,
    launcher_id: &[u8; 32],
    inner_puzzle: NodePtr,
) -> Result<NodePtr, EvalErr> {
    let singleton_hash = a.new_atom(&SINGLETON_TOP_LAYER_HASH)?;
    let launcher_id = a.new_atom(launcher_id)?;
    let launcher_hash = a.new_atom(&SINGLETON_LAUNCHER_HASH)?;

    let singleton_struct = a.new_pair(launcher_id, launcher_hash)?;
    let singleton_struct = a.new_pair(singleton_hash, singleton_struct)?;

    curry(a, node, &[singleton_struct, inner_puzzle])
}

pub fn solve_singleton(
    a: &mut Allocator,
    lineage_proof: &Proof,
    amount: u64,
    inner_solution: NodePtr,
) -> Result<NodePtr, Error> {
    let lineage_proof_ptr = lineage_proof.to_clvm(a)?;
    let amount_ptr = a.new_number(amount.into()).map_err(Error::Eval)?;

    new_list(a, &[lineage_proof_ptr, amount_ptr, inner_solution]).map_err(Error::Eval)
}
