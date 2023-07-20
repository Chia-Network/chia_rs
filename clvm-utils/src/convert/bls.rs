use chia_bls::PublicKey;
use clvmr::{allocator::NodePtr, Allocator};

use crate::{Error, FromClvm, Result, ToClvm};

impl ToClvm for PublicKey {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        self.to_bytes().to_clvm(a)
    }
}

impl FromClvm for PublicKey {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        Self::from_bytes(&<[u8; 48]>::from_clvm(a, node)?)
            .ok_or(Error::Reason("could not parse public key".to_string()))
    }
}
