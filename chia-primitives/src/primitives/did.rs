use clvm_utils::{clvm_list, match_list, Error, FromClvm, Result, ToClvm};
use clvmr::{
    allocator::{NodePtr, SExp},
    Allocator,
};

use crate::singleton::SingletonStruct;

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(curried_args)]
pub struct Did<T, M> {
    pub inner_puzzle: T,
    pub recovery_did_list_hash: [u8; 32],
    pub num_verifications_required: u64,
    pub singleton_struct: SingletonStruct,
    pub metadata: M,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DidSolution<T> {
    InnerSpend(T),
}

impl<T: FromClvm> FromClvm for DidSolution<T> {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        match a.sexp(node) {
            SExp::Atom() => Err(Error::ExpectedCons(node)),
            SExp::Pair(first, rest) => match first {
                1 => Ok(Self::InnerSpend(<match_list!(T)>::from_clvm(a, rest)?.0)),
            },
        }
    }
}

impl<T: ToClvm> ToClvm for DidSolution<T> {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        match self {
            Self::InnerSpend(solution) => clvm_list!(1, solution).to_clvm(a),
        }
    }
}
