use chia_protocol::{Bytes, Bytes32};
use clvm_traits::{FromClvm, ToClvm};
use clvm_utils::TreeHash;
use hex_literal::hex;

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(transparent)]
pub struct SettlementPaymentsSolution {
    pub notarized_payments: Vec<NotarizedPayment>,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct NotarizedPayment {
    pub nonce: Bytes32,
    #[clvm(rest)]
    pub payments: Vec<Payment>,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct Payment {
    pub puzzle_hash: Bytes32,
    pub amount: u64,
    #[clvm(default)]
    pub memos: Vec<Bytes>,
}

impl Payment {
    pub fn new(puzzle_hash: Bytes32, amount: u64) -> Self {
        Self::with_memos(puzzle_hash, amount, Vec::new())
    }

    pub fn with_memos(puzzle_hash: Bytes32, amount: u64, memos: Vec<Bytes>) -> Self {
        Self {
            puzzle_hash,
            amount,
            memos,
        }
    }
}

/// This is the puzzle reveal of the [offer settlement payments](https://chialisp.com/offers) puzzle.
pub const SETTLEMENT_PAYMENTS_PUZZLE: [u8; 293] = hex!(
    "
    ff02ffff01ff02ff0affff04ff02ffff04ff03ff80808080ffff04ffff01ffff
    333effff02ffff03ff05ffff01ff04ffff04ff0cffff04ffff02ff1effff04ff
    02ffff04ff09ff80808080ff808080ffff02ff16ffff04ff02ffff04ff19ffff
    04ffff02ff0affff04ff02ffff04ff0dff80808080ff808080808080ff8080ff
    0180ffff02ffff03ff05ffff01ff02ffff03ffff15ff29ff8080ffff01ff04ff
    ff04ff08ff0980ffff02ff16ffff04ff02ffff04ff0dffff04ff0bff80808080
    8080ffff01ff088080ff0180ffff010b80ff0180ff02ffff03ffff07ff0580ff
    ff01ff0bffff0102ffff02ff1effff04ff02ffff04ff09ff80808080ffff02ff
    1effff04ff02ffff04ff0dff8080808080ffff01ff0bffff0101ff058080ff01
    80ff018080
    "
);

/// This is the puzzle hash of the [offer settlement payments](https://chialisp.com/offers) puzzle.
pub const SETTLEMENT_PAYMENTS_PUZZLE_HASH: TreeHash = TreeHash::new(hex!(
    "
    cfbfdeed5c4ca2de3d0bf520b9cb4bb7743a359bd2e6a188d19ce7dffc21d3e7
    "
));

/// This is the puzzle reveal of the old [offer settlement payments](https://chialisp.com/offers) puzzle.
///
/// **Warning:**
/// It is recommended not to use settlement payments v1 for anything other than backwards compatibility (e.g. offer compression).
pub const SETTLEMENT_PAYMENTS_PUZZLE_V1: [u8; 267] = hex!(
    "
    ff02ffff01ff02ff0affff04ff02ffff04ff03ff80808080ffff04ffff01ffff
    333effff02ffff03ff05ffff01ff04ffff04ff0cffff04ffff02ff1effff04ff
    02ffff04ff09ff80808080ff808080ffff02ff16ffff04ff02ffff04ff19ffff
    04ffff02ff0affff04ff02ffff04ff0dff80808080ff808080808080ff8080ff
    0180ffff02ffff03ff05ffff01ff04ffff04ff08ff0980ffff02ff16ffff04ff
    02ffff04ff0dffff04ff0bff808080808080ffff010b80ff0180ff02ffff03ff
    ff07ff0580ffff01ff0bffff0102ffff02ff1effff04ff02ffff04ff09ff8080
    8080ffff02ff1effff04ff02ffff04ff0dff8080808080ffff01ff0bffff0101
    ff058080ff0180ff018080
    "
);

/// This is the puzzle hash of the old [offer settlement payments](https://chialisp.com/offers) puzzle.
///
/// **Warning:**
/// It is recommended not to use settlement payments v1 for anything other than backwards compatibility (e.g. offer compression).
pub const SETTLEMENT_PAYMENTS_PUZZLE_HASH_V1: TreeHash = TreeHash::new(hex!(
    "
    bae24162efbd568f89bc7a340798a6118df0189eb9e3f8697bcea27af99f8f79
    "
));

#[cfg(test)]
mod tests {
    use super::*;

    use crate::assert_puzzle_hash;

    #[test]
    fn puzzle_hashes() {
        assert_puzzle_hash!(SETTLEMENT_PAYMENTS_PUZZLE => SETTLEMENT_PAYMENTS_PUZZLE_HASH);
        assert_puzzle_hash!(SETTLEMENT_PAYMENTS_PUZZLE_V1 => SETTLEMENT_PAYMENTS_PUZZLE_HASH_V1);
    }
}
