use crate::{Bytes32, BytesImpl};
use chia_sha2::Sha256;
use chia_streamable_macro::streamable;
use clvm_traits::{
    clvm_list, destructure_list, match_list, ClvmDecoder, ClvmEncoder, FromClvm, FromClvmError,
    ToClvm, ToClvmError,
};

#[cfg(feature = "py-bindings")]
use pyo3::exceptions::PyNotImplementedError;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyType;

#[streamable]
#[derive(Copy)]
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
        if self.amount >= 0x8000_0000_0000_0000_u64 {
            hasher.update([0_u8]);
            hasher.update(amount_bytes);
        } else {
            let start = match self.amount {
                n if n >= 0x0080_0000_0000_0000_u64 => 0,
                n if n >= 0x8000_0000_0000_u64 => 1,
                n if n >= 0x0080_0000_0000_u64 => 2,
                n if n >= 0x8000_0000_u64 => 3,
                n if n >= 0x0080_0000_u64 => 4,
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
    fn name<'p>(&self, py: Python<'p>) -> Bound<'p, pyo3::types::PyBytes> {
        pyo3::types::PyBytes::new_bound(py, &self.coin_id())
    }
}

#[cfg(feature = "py-bindings")]
#[pymethods]
impl Coin {
    #[classmethod]
    #[pyo3(name = "from_parent")]
    pub fn from_parent(_cls: &Bound<'_, PyType>, _coin: Self) -> PyResult<PyObject> {
        Err(PyNotImplementedError::new_err(
            "Coin does not support from_parent().",
        ))
    }
}

impl<N, E: ClvmEncoder<Node = N>> ToClvm<E> for Coin {
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        clvm_list!(self.parent_coin_info, self.puzzle_hash, self.amount).to_clvm(encoder)
    }
}

impl<N, D: ClvmDecoder<Node = N>> FromClvm<D> for Coin {
    fn from_clvm(decoder: &D, node: N) -> Result<Self, FromClvmError> {
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
    #[case(0x00ff_ffff, &[0, 0xff, 0xff, 0xff])]
    #[case(0xffff_ffff, &[0, 0xff, 0xff, 0xff, 0xff])]
    #[case(0x00ff_ffff_ffff, &[0, 0xff, 0xff, 0xff, 0xff, 0xff])]
    #[case(0xffff_ffff_ffff_ffff, &[0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff])]
    #[case(0x7f, &[0x7f])]
    #[case(0x7fff, &[0x7f, 0xff])]
    #[case(0x007f_ffff, &[0x7f, 0xff, 0xff])]
    #[case(0x7fff_ffff, &[0x7f, 0xff, 0xff, 0xff])]
    #[case(0x007f_ffff_ffff, &[0x7f, 0xff, 0xff, 0xff, 0xff])]
    #[case(0x7fff_ffff_ffff_ffff, &[0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff])]
    #[case(0x80, &[0, 0x80])]
    #[case(0x8000, &[0, 0x80, 0x00])]
    #[case(0x0080_0000, &[0, 0x80, 0x00, 0x00])]
    #[case(0x8000_0000, &[0, 0x80, 0x00, 0x00, 0x00])]
    #[case(0x0080_0000_0000, &[0, 0x80, 0x00, 0x00, 0x00, 0x00])]
    #[case(0x8000_0000_0000_0000, &[0, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])]
    fn coin_id(#[case] amount: u64, #[case] bytes: &[u8]) {
        let parent_coin = b"---foo---                       ";
        let puzzle_hash = b"---bar---                       ";

        let c = Coin::new(parent_coin.into(), puzzle_hash.into(), amount);
        let mut sha256 = Sha256::new();
        sha256.update(parent_coin);
        sha256.update(puzzle_hash);
        sha256.update(bytes);
        assert_eq!(c.coin_id().to_bytes(), sha256.finalize().as_ref());
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
