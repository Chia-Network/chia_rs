use chia_protocol::Bytes32;
use clvm_traits::{FromClvm, ToClvm};

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(fuzzing, derive(arbitrary::Arbitrary))]
#[clvm(untagged, tuple)]
pub enum Proof {
    Lineage(LineageProof),
    Eve(EveProof),
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(fuzzing, derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct LineageProof {
    pub parent_coin_info: Bytes32,
    pub inner_puzzle_hash: Bytes32,
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(fuzzing, derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct EveProof {
    pub parent_coin_info: Bytes32,
    pub amount: u64,
}
