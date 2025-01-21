use chia_protocol::{Bytes, Bytes32};
use clvm_traits::{FromClvm, ToClvm};

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
    /// The memos should usually be set to [`None`] instead of an empty list.
    /// This is for compatibility with the way the reference wallet encodes offers.
    #[clvm(rest)]
    pub memos: Option<Memos>,
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct Memos(pub Vec<Bytes>);

impl Payment {
    pub fn new(puzzle_hash: Bytes32, amount: u64) -> Self {
        Self {
            puzzle_hash,
            amount,
            memos: None,
        }
    }

    pub fn with_memos(puzzle_hash: Bytes32, amount: u64, memos: Vec<Bytes>) -> Self {
        Self {
            puzzle_hash,
            amount,
            memos: Some(Memos(memos)),
        }
    }
}

#[cfg(test)]
mod tests {
    use clvm_utils::tree_hash;
    use clvmr::{serde::node_from_bytes, Allocator};
    use hex_literal::hex;

    use super::*;

    #[test]
    fn test_empty_memos() -> anyhow::Result<()> {
        let mut allocator = Allocator::new();

        /*
        ((0xd951714bbcd0d0af317b3ef432472b57e7c48d3036b4491539c186ce1377cad2
            (0x2a5cbc6f5076e0517bdb1e4664b3c26e64d27178b65aaa1ae97267eee629113b 0x04a817c800 ())
        ))
        */
        let expected_payment = node_from_bytes(
            &mut allocator,
            &hex!(
                "
                ffffa0d951714bbcd0d0af317b3ef432472b57e7c48d3036b4491539c186ce13
                77cad2ffffa02a5cbc6f5076e0517bdb1e4664b3c26e64d27178b65aaa1ae972
                67eee629113bff8504a817c800ff80808080
                "
            ),
        )?;

        let nonce = Bytes32::from(hex!(
            "d951714bbcd0d0af317b3ef432472b57e7c48d3036b4491539c186ce1377cad2"
        ));
        let puzzle_hash = Bytes32::from(hex!(
            "2a5cbc6f5076e0517bdb1e4664b3c26e64d27178b65aaa1ae97267eee629113b"
        ));
        let amount = 20_000_000_000;
        let memos = Vec::new();

        let payment = Payment::with_memos(puzzle_hash, amount, memos);
        let notarized_payment = SettlementPaymentsSolution {
            notarized_payments: vec![NotarizedPayment {
                nonce,
                payments: vec![payment],
            }],
        }
        .to_clvm(&mut allocator)?;

        assert_eq!(
            tree_hash(&allocator, notarized_payment),
            tree_hash(&allocator, expected_payment)
        );

        Ok(())
    }

    #[test]
    fn test_missing_memos() -> anyhow::Result<()> {
        let mut allocator = Allocator::new();

        /*
        ((0xd951714bbcd0d0af317b3ef432472b57e7c48d3036b4491539c186ce1377cad2
            (0x2a5cbc6f5076e0517bdb1e4664b3c26e64d27178b65aaa1ae97267eee629113b 0x04a817c800)
        ))
        */
        let expected_payment = node_from_bytes(
            &mut allocator,
            &hex!(
                "
                ffffa0d951714bbcd0d0af317b3ef432472b57e7c48d3036b4491539c186ce13
                77cad2ffffa02a5cbc6f5076e0517bdb1e4664b3c26e64d27178b65aaa1ae972
                67eee629113bff8504a817c800808080
                "
            ),
        )?;

        let nonce = Bytes32::from(hex!(
            "d951714bbcd0d0af317b3ef432472b57e7c48d3036b4491539c186ce1377cad2"
        ));
        let puzzle_hash = Bytes32::from(hex!(
            "2a5cbc6f5076e0517bdb1e4664b3c26e64d27178b65aaa1ae97267eee629113b"
        ));
        let amount = 20_000_000_000;

        let payment = Payment::new(puzzle_hash, amount);
        let notarized_payment = SettlementPaymentsSolution {
            notarized_payments: vec![NotarizedPayment {
                nonce,
                payments: vec![payment],
            }],
        }
        .to_clvm(&mut allocator)?;

        assert_eq!(
            tree_hash(&allocator, notarized_payment),
            tree_hash(&allocator, expected_payment)
        );

        Ok(())
    }
}
