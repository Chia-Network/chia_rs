use arbitrary::{Arbitrary, Unstructured};
use clvm_traits::{FromClvm, Result, ToClvm};
use clvmr::{allocator::NodePtr, Allocator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proof {
    Lineage(LineageProof),
    Eve(EveProof),
}

impl FromClvm for Proof {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        LineageProof::from_clvm(a, node)
            .map(Self::Lineage)
            .or_else(|_| EveProof::from_clvm(a, node).map(Self::Eve))
    }
}

impl ToClvm for Proof {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        match self {
            Self::Lineage(lineage_proof) => lineage_proof.to_clvm(a),
            Self::Eve(eve_proof) => eve_proof.to_clvm(a),
        }
    }
}

impl<'a> Arbitrary<'a> for Proof {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let is_eve = u.ratio(3, 10)?;
        if is_eve {
            Ok(Self::Eve(EveProof {
                parent_coin_info: u.arbitrary()?,
                amount: u.arbitrary()?,
            }))
        } else {
            Ok(Self::Lineage(LineageProof {
                parent_coin_info: u.arbitrary()?,
                inner_puzzle_hash: u.arbitrary()?,
                amount: u.arbitrary()?,
            }))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(proper_list)]
pub struct LineageProof {
    pub parent_coin_info: [u8; 32],
    pub inner_puzzle_hash: [u8; 32],
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(proper_list)]
pub struct EveProof {
    pub parent_coin_info: [u8; 32],
    pub amount: u64,
}
