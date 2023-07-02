use crate::bytes::Bytes32;
use crate::chia_error;
use crate::streamable::Streamable;
use crate::streamable_struct;
use chia_streamable_macro::Streamable;
use sha2::{Digest, Sha256};
use std::convert::TryInto;

#[cfg(feature = "py-bindings")]
use crate::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use crate::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;
#[cfg(feature = "py-bindings")]
use pyo3::types::PyBytes;

streamable_struct!(Coin {
    parent_coin_info: Bytes32,
    puzzle_hash: Bytes32,
    amount: u64,
});

impl Coin {
    pub fn coin_id(&self) -> [u8; 32] {
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

        hasher.finalize().as_slice().try_into().unwrap()
    }
}

#[cfg(feature = "py-bindings")]
#[cfg_attr(feature = "py-bindings", pymethods)]
impl Coin {
    fn name<'p>(&self, py: Python<'p>) -> PyResult<&'p PyBytes> {
        Ok(PyBytes::new(py, &self.coin_id()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(c.coin_id(), &sha256.finalize() as &[u8]);
    }
}
