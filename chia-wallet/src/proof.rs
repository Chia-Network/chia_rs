use arbitrary::{Arbitrary, Unstructured};
use clvm_traits::{ClvmDecoder, ClvmEncoder, FromClvm, FromClvmError, ToClvm, ToClvmError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proof {
    Lineage(LineageProof),
    Eve(EveProof),
}

impl<N> FromClvm<N> for Proof {
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, node: N) -> Result<Self, FromClvmError> {
        LineageProof::from_clvm(decoder, decoder.clone_node(&node))
            .map(Self::Lineage)
            .or_else(|_| EveProof::from_clvm(decoder, node).map(Self::Eve))
    }
}

impl<N> ToClvm<N> for Proof {
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        match self {
            Self::Lineage(lineage_proof) => lineage_proof.to_clvm(encoder),
            Self::Eve(eve_proof) => eve_proof.to_clvm(encoder),
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
#[clvm(list)]
pub struct LineageProof {
    pub parent_coin_info: [u8; 32],
    pub inner_puzzle_hash: [u8; 32],
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(list)]
pub struct EveProof {
    pub parent_coin_info: [u8; 32],
    pub amount: u64,
}
