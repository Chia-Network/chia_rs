use crate::{Bytes32, BytesImpl};
use chia_streamable_macro::streamable;
use clvm_traits::{
    clvm_list, destructure_list, match_list, ClvmDecoder, ClvmEncoder, FromClvm, FromClvmError,
    ToClvm, ToClvmError,
};
use sha2::{Digest, Sha256};

#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

#[streamable]
pub struct Coin {
    parent_coin_info: Bytes32,
    puzzle_hash: Bytes32,
    amount: u64,
}

impl Coin {
    pub fn coin_id(&self) -> Bytes32 {
        let mut hasher = Sha256::new();
        hasher.update(self.parent_coin_info);
        hasher.update(self.puzzle_hash);

        let amount_bytes = self.amount.to_be_bytes();
        if self.amount >= 0x8000000000000000_u64 {
            hasher.update([0_u8]);
            hasher.update(amount_bytes);
        } else {
            let start = match self.amount {
                n if n >= 0x80000000000000_u64 => 0,
                n if n >= 0x800000000000_u64 => 1,
                n if n >= 0x8000000000_u64 => 2,
                n if n >= 0x80000000_u64 => 3,
                n if n >= 0x800000_u64 => 4,
                n if n >= 0x8000_u64 => 5,
                n if n >= 0x80_u64 => 6,
                n if n > 0 => 7,
                _ => 8,
            };
            hasher.update(&amount_bytes[start..]);
        }

        let coin_id: [u8; 32] = hasher.finalize().as_slice().try_into().unwrap();
        Bytes32::new(coin_id)
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl Coin {
    fn name<'p>(&self, py: pyo3::Python<'p>) -> pyo3::PyResult<&'p pyo3::types::PyBytes> {
        Ok(pyo3::types::PyBytes::new(py, &self.coin_id()))
    }
}

impl<N> ToClvm<N> for Coin {
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        clvm_list!(self.parent_coin_info, self.puzzle_hash, self.amount).to_clvm(encoder)
    }
}

impl<N> FromClvm<N> for Coin {
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, node: N) -> Result<Self, FromClvmError> {
        let destructure_list!(parent_coin_info, puzzle_hash, amount) =
            <match_list!(BytesImpl<32>, BytesImpl<32>, u64)>::from_clvm(decoder, node)?;
        Ok(Coin {
            parent_coin_info,
            puzzle_hash,
            amount,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clvmr::{
        serde::{node_from_bytes, node_to_bytes},
        Allocator,
    };
    use rstest::rstest;

    #[rstest]
    #[case(0, &[])]
    #[case(1, &[1])]
    #[case(0xff, &[0, 0xff])]
    #[case(0xffff, &[0, 0xff, 0xff])]
    #[case(0xffffff, &[0, 0xff, 0xff, 0xff])]
    #[case(0xffffffff, &[0, 0xff, 0xff, 0xff, 0xff])]
    #[case(0xffffffffff, &[0, 0xff, 0xff, 0xff, 0xff, 0xff])]
    #[case(0xffffffffffffffff, &[0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff])]
    #[case(0x7f, &[0x7f])]
    #[case(0x7fff, &[0x7f, 0xff])]
    #[case(0x7fffff, &[0x7f, 0xff, 0xff])]
    #[case(0x7fffffff, &[0x7f, 0xff, 0xff, 0xff])]
    #[case(0x7fffffffff, &[0x7f, 0xff, 0xff, 0xff, 0xff])]
    #[case(0x7fffffffffffffff, &[0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff])]
    #[case(0x80, &[0, 0x80])]
    #[case(0x8000, &[0, 0x80, 0x00])]
    #[case(0x800000, &[0, 0x80, 0x00, 0x00])]
    #[case(0x80000000, &[0, 0x80, 0x00, 0x00, 0x00])]
    #[case(0x8000000000, &[0, 0x80, 0x00, 0x00, 0x00, 0x00])]
    #[case(0x8000000000000000, &[0, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])]
    fn coin_id(#[case] amount: u64, #[case] bytes: &[u8]) {
        let parent_coin = b"---foo---                       ";
        let puzzle_hash = b"---bar---                       ";

        let c = Coin::new(parent_coin.into(), puzzle_hash.into(), amount);
        let mut sha256 = Sha256::new();
        sha256.update(parent_coin);
        sha256.update(puzzle_hash);
        sha256.update(bytes);
        assert_eq!(c.coin_id().to_bytes(), &sha256.finalize() as &[u8]);
    }

    #[test]
    fn coin_roundtrip() {
        let a = &mut Allocator::new();
        let expected = "ffa09e144397decd2b831551f9710c17ae776d9c5a3ae5283c5f9747263fd1255381ffa0eff07522495060c066f66f32acc2a77e3a3e737aca8baea4d1a64ea4cdc13da9ff0180";
        let expected_bytes = hex::decode(expected).unwrap();

        let ptr = node_from_bytes(a, &expected_bytes).unwrap();
        let coin = Coin::from_clvm(a, ptr).unwrap();

        let round_trip = coin.to_clvm(a).unwrap();
        assert_eq!(expected, hex::encode(node_to_bytes(a, round_trip).unwrap()));
    }
}
