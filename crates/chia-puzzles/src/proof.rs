use chia_protocol::Bytes32;
use clvm_traits::{FromClvm, ToClvm};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(untagged, tuple)]
pub enum Proof {
    Lineage(LineageProof),
    Eve(EveProof),
}

impl Proof {
    pub fn lineage(
        parent_parent_coin_id: Bytes32,
        parent_inner_puzzle_hash: Bytes32,
        parent_amount: u64,
    ) -> Self {
        Self::Lineage(LineageProof::new(
            parent_parent_coin_id,
            parent_inner_puzzle_hash,
            parent_amount,
        ))
    }

    pub fn eve(parent_coin_info: Bytes32, amount: u64) -> Self {
        Self::Eve(EveProof::new(parent_coin_info, amount))
    }

    pub fn is_lineage(&self) -> bool {
        matches!(self, Self::Lineage(_))
    }

    pub fn is_eve(&self) -> bool {
        matches!(self, Self::Eve(_))
    }

    pub fn as_lineage(&self) -> Option<LineageProof> {
        match self {
            Self::Lineage(proof) => Some(*proof),
            _ => None,
        }
    }

    pub fn as_eve(&self) -> Option<EveProof> {
        match self {
            Self::Eve(proof) => Some(*proof),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct LineageProof {
    pub parent_parent_coin_id: Bytes32,
    pub parent_inner_puzzle_hash: Bytes32,
    pub parent_amount: u64,
}

impl LineageProof {
    pub fn new(
        parent_parent_coin_id: Bytes32,
        parent_inner_puzzle_hash: Bytes32,
        parent_amount: u64,
    ) -> Self {
        Self {
            parent_parent_coin_id,
            parent_inner_puzzle_hash,
            parent_amount,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct EveProof {
    pub parent_coin_info: Bytes32,
    pub amount: u64,
}

impl EveProof {
    pub fn new(parent_coin_info: Bytes32, amount: u64) -> Self {
        Self {
            parent_coin_info,
            amount,
        }
    }
}
