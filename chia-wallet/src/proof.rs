use arbitrary::{Arbitrary, Unstructured};
use chia_protocol::Bytes32;
use clvm_traits::{from_clvm, to_clvm, FromClvm, ToClvm};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proof {
    Lineage(LineageProof),
    Eve(EveProof),
}

impl<Node> FromClvm<Node> for Proof
where
    Node: Clone,
{
    from_clvm!(Node, f, ptr, {
        LineageProof::from_clvm(f, ptr.clone())
            .map(Self::Lineage)
            .or_else(|_| EveProof::from_clvm(f, ptr).map(Self::Eve))
    });
}

impl<Node> ToClvm<Node> for Proof
where
    Node: Clone,
{
    to_clvm!(Node, self, f, {
        match self {
            Self::Lineage(lineage_proof) => lineage_proof.to_clvm(f),
            Self::Eve(eve_proof) => eve_proof.to_clvm(f),
        }
    });
}

impl<'a> Arbitrary<'a> for Proof {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let is_eve = u.ratio(3, 10)?;
        if is_eve {
            Ok(Self::Eve(EveProof {
                parent_coin_info: u.arbitrary::<[u8; 32]>()?.into(),
                amount: u.arbitrary()?,
            }))
        } else {
            Ok(Self::Lineage(LineageProof {
                parent_coin_info: u.arbitrary::<[u8; 32]>()?.into(),
                inner_puzzle_hash: u.arbitrary::<[u8; 32]>()?.into(),
                amount: u.arbitrary()?,
            }))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(list)]
pub struct LineageProof {
    pub parent_coin_info: Bytes32,
    pub inner_puzzle_hash: Bytes32,
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(list)]
pub struct EveProof {
    pub parent_coin_info: Bytes32,
    pub amount: u64,
}
