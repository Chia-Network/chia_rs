use clvm_utils::{clvm_list, match_list, match_tuple, Error, FromClvm, LazyNode, Result, ToClvm};
use clvmr::{allocator::NodePtr, Allocator};

use crate::SingletonStruct;

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct DidArgs {
    pub inner_puzzle: LazyNode,
    pub recovery_did_list_hash: [u8; 32],
    pub num_verifications_required: u64,
    pub singleton_struct: SingletonStruct,
    pub metadata: LazyNode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DidSolution {
    InnerSpend(LazyNode),
}

impl FromClvm for DidSolution {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        let (mode, LazyNode(args)) = <match_tuple!(u8, LazyNode)>::from_clvm(a, node)?;

        match mode {
            1 => Ok(Self::InnerSpend(
                <match_list!(LazyNode)>::from_clvm(a, args)?.0,
            )),
            _ => Err(Error::Reason(format!("unexpected did spend mode {}", mode))),
        }
    }
}

impl ToClvm for DidSolution {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        match self {
            Self::InnerSpend(solution) => clvm_list!(1, solution).to_clvm(a),
        }
    }
}
