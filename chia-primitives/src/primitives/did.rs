use clvm_utils::{new_list, FromClvm, ToClvm};
use clvmr::{allocator::NodePtr, reduction::EvalErr, Allocator};

use crate::singleton::SingletonStruct;

#[derive(Debug, Clone, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct Did<T, M> {
    pub inner_puzzle: T,
    pub recovery_did_list_hash: [u8; 32],
    pub num_verifications_required: u64,
    pub singleton_struct: SingletonStruct,
    pub metadata: M,
}

pub fn did_solution(a: &mut Allocator, inner_solution: NodePtr) -> Result<NodePtr, EvalErr> {
    let mode = a.one();
    new_list(a, &[mode, inner_solution])
}
