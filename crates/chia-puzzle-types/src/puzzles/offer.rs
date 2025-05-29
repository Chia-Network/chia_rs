use chia_protocol::Bytes32;
use clvm_traits::{FromClvm, ToClvm};
use clvmr::NodePtr;

use crate::Memos;

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(transparent)]
pub struct SettlementPaymentsSolution<T = NodePtr> {
    pub notarized_payments: Vec<NotarizedPayment<T>>,
}

impl SettlementPaymentsSolution {
    pub fn new(notarized_payments: Vec<NotarizedPayment>) -> Self {
        Self { notarized_payments }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct NotarizedPayment<T = NodePtr> {
    pub nonce: Bytes32,
    #[clvm(rest)]
    pub payments: Vec<Payment<T>>,
}

impl NotarizedPayment {
    pub fn new(nonce: Bytes32, payments: Vec<Payment>) -> Self {
        Self { nonce, payments }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, ToClvm, FromClvm)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[clvm(list)]
pub struct Payment<T = NodePtr> {
    pub puzzle_hash: Bytes32,
    pub amount: u64,
    #[clvm(rest)]
    pub memos: Memos<T>,
}

impl Payment {
    pub fn new(puzzle_hash: Bytes32, amount: u64, memos: Memos) -> Self {
        Self {
            puzzle_hash,
            amount,
            memos,
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
        let expected_solution = node_from_bytes(
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

        let payment = Payment::new(puzzle_hash, amount, Memos::Some(NodePtr::NIL));
        let solution = SettlementPaymentsSolution::new(vec![NotarizedPayment {
            nonce,
            payments: vec![payment],
        }])
        .to_clvm(&mut allocator)?;

        assert_eq!(
            tree_hash(&allocator, solution),
            tree_hash(&allocator, expected_solution)
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
        let expected_solution = node_from_bytes(
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

        let payment = Payment::new(puzzle_hash, amount, Memos::<NodePtr>::None);
        let solution = SettlementPaymentsSolution::new(vec![NotarizedPayment {
            nonce,
            payments: vec![payment],
        }])
        .to_clvm(&mut allocator)?;

        assert_eq!(
            tree_hash(&allocator, solution),
            tree_hash(&allocator, expected_solution)
        );

        Ok(())
    }
}
