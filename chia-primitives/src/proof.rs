use clvm_utils::Allocate;
use clvmr::{allocator::NodePtr, Allocator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proof {
    Lineage(LineageProof),
    Eve(EveProof),
}

impl Allocate for Proof {
    fn from_clvm(a: &Allocator, node: NodePtr) -> clvm_utils::Result<Self> {
        LineageProof::from_clvm(a, node)
            .map(Self::Lineage)
            .or_else(|_| EveProof::from_clvm(a, node).map(Self::Eve))
    }
    fn to_clvm(&self, a: &mut Allocator) -> clvm_utils::Result<NodePtr> {
        match self {
            Self::Lineage(lineage_proof) => lineage_proof.to_clvm(a),
            Self::Eve(eve_proof) => eve_proof.to_clvm(a),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageProof {
    pub parent_coin_info: [u8; 32],
    pub inner_puzzle_hash: [u8; 32],
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EveProof {
    pub parent_coin_info: [u8; 32],
    pub amount: u64,
}

impl Allocate for LineageProof {
    fn from_clvm(a: &Allocator, node: NodePtr) -> clvm_utils::Result<Self> {
        let value = <([u8; 32], ([u8; 32], (u64, [u8; 0])))>::from_clvm(a, node)?;
        Ok(Self {
            parent_coin_info: value.0,
            inner_puzzle_hash: value.1 .0,
            amount: value.1 .1 .0,
        })
    }
    fn to_clvm(&self, a: &mut Allocator) -> clvm_utils::Result<NodePtr> {
        (
            self.parent_coin_info,
            (self.inner_puzzle_hash, (self.amount, [0u8; 0])),
        )
            .to_clvm(a)
    }
}

impl Allocate for EveProof {
    fn from_clvm(a: &Allocator, node: NodePtr) -> clvm_utils::Result<Self> {
        let value = <([u8; 32], (u64, [u8; 0]))>::from_clvm(a, node)?;
        Ok(Self {
            parent_coin_info: value.0,
            amount: value.1 .0,
        })
    }
    fn to_clvm(&self, a: &mut Allocator) -> clvm_utils::Result<NodePtr> {
        (self.parent_coin_info, (self.amount, [0u8; 0])).to_clvm(a)
    }
}
