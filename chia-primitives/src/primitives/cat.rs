use clvm_utils::{FromClvm, LazyNode, ToClvm};

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct Cat {
    pub mod_hash: [u8; 32],
    pub tail_program_hash: [u8; 32],
    pub inner_puzzle: LazyNode,
}
