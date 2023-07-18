use clvm_utils::Allocate;
use clvmr::{allocator::NodePtr, reduction::EvalErr, Allocator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageProof {
    pub parent_coin_info: [u8; 32],
    pub inner_puzzle_hash: [u8; 32],
    pub amount: u64,
}

impl Allocate for LineageProof {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Option<Self> {
        let value = <([u8; 32], ([u8; 32], u64))>::from_clvm(a, node)?;
        Some(Self {
            parent_coin_info: value.0,
            inner_puzzle_hash: value.1 .0,
            amount: value.1 .1,
        })
    }
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr, EvalErr> {
        (self.parent_coin_info, (self.inner_puzzle_hash, self.amount)).to_clvm(a)
    }
}

pub fn alloc_lineage_proof(
    a: &mut Allocator,
    lineage_proof: &LineageProof,
) -> Result<NodePtr, EvalErr> {
    let parent_coin_info = a.new_atom(&lineage_proof.parent_coin_info)?;
    let inner_puzzle_hash = a.new_atom(&lineage_proof.inner_puzzle_hash)?;
    let amount = a.new_number(lineage_proof.amount.into())?;

    let rest = a.new_pair(inner_puzzle_hash, amount)?;
    a.new_pair(parent_coin_info, rest)
}
