use clvm_utils::{new_list, Error, FromClvm, ToClvm};
use clvmr::{allocator::NodePtr, Allocator};

use crate::Proof;

#[derive(Debug, Clone, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct Singleton<T> {
    pub singleton_struct: SingletonStruct,
    pub inner_puzzle: T,
}

#[derive(Debug, Clone, ToClvm, FromClvm)]
#[clvm(tuple)]
pub struct SingletonStruct {
    pub mod_hash: [u8; 32],
    pub launcher_id: [u8; 32],
    pub launcher_puzzle_hash: [u8; 32],
}

pub fn singleton_solution(
    a: &mut Allocator,
    proof: &Proof,
    amount: u64,
    inner_solution: NodePtr,
) -> Result<NodePtr, Error> {
    let lineage_proof_ptr = proof.to_clvm(a)?;
    let amount_ptr = a.new_number(amount.into()).map_err(Error::Allocator)?;
    new_list(a, &[lineage_proof_ptr, amount_ptr, inner_solution]).map_err(Error::Allocator)
}
