use crate::consensus_constants::ConsensusConstants;
use crate::opcodes::{
    ConditionOpcode, AGG_SIG_AMOUNT, AGG_SIG_ME, AGG_SIG_PARENT, AGG_SIG_PARENT_AMOUNT,
    AGG_SIG_PARENT_PUZZLE, AGG_SIG_PUZZLE, AGG_SIG_PUZZLE_AMOUNT,
};
use crate::owned_conditions::OwnedSpendConditions;
use chia_protocol::Coin;

pub fn make_aggsig_final_message(
    opcode: ConditionOpcode,
    msg: &mut Vec<u8>,
    spend: &OwnedSpendConditions,
    constants: &ConsensusConstants,
) {
    match opcode {
        AGG_SIG_PARENT => {
            msg.extend(spend.parent_id.as_slice());
            msg.extend(constants.agg_sig_parent_additional_data.as_slice());
        }
        AGG_SIG_PUZZLE => {
            msg.extend(spend.puzzle_hash.as_slice());
            msg.extend(constants.agg_sig_puzzle_additional_data.as_slice());
        }
        AGG_SIG_AMOUNT => {
            msg.extend(u64_to_bytes(spend.coin_amount).as_slice());
            msg.extend(constants.agg_sig_amount_additional_data.as_slice());
        }
        AGG_SIG_PUZZLE_AMOUNT => {
            msg.extend(spend.puzzle_hash.as_slice());
            msg.extend(u64_to_bytes(spend.coin_amount).as_slice());
            msg.extend(constants.agg_sig_puzzle_amount_additional_data.as_slice());
        }
        AGG_SIG_PARENT_AMOUNT => {
            msg.extend(spend.parent_id.as_slice());
            msg.extend(u64_to_bytes(spend.coin_amount).as_slice());
            msg.extend(constants.agg_sig_parent_amount_additional_data.as_slice());
        }
        AGG_SIG_PARENT_PUZZLE => {
            msg.extend(spend.parent_id.as_slice());
            msg.extend(spend.puzzle_hash.as_slice());
            msg.extend(constants.agg_sig_parent_puzzle_additional_data.as_slice());
        }
        AGG_SIG_ME => {
            let coin: Coin = Coin::new(spend.parent_id, spend.puzzle_hash, spend.coin_amount);

            msg.extend(coin.coin_id().as_slice());
            msg.extend(constants.agg_sig_me_additional_data.as_slice());
        }
        _ => {}
    }
}

pub fn u64_to_bytes(val: u64) -> Vec<u8> {
    let amount_bytes: [u8; 8] = val.to_be_bytes();
    if val >= 0x8000_0000_0000_0000_u64 {
        let mut ret = Vec::<u8>::new();
        ret.push(0_u8);
        ret.extend(amount_bytes);
        ret
    } else {
        let start = match val {
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
        amount_bytes[start..].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocator::make_allocator;
    use crate::consensus_constants::TEST_CONSTANTS;
    use clvmr::chia_dialect::LIMIT_HEAP;
    use clvmr::Allocator;
    use hex_literal::hex;
    use rstest::rstest;

    #[test]
    fn test_validate_u64() {
        let mut a: Allocator = make_allocator(LIMIT_HEAP);
        for v in 0..10000 {
            let ptr = a.new_small_number(v).expect("valid u64");
            assert_eq!(a.atom(ptr).as_ref(), u64_to_bytes(v as u64).as_slice());
        }
        for v in 18_446_744_073_709_551_615_u64 - 1000..18_446_744_073_709_551_615 {
            let ptr = a.new_number(v.into()).expect("valid u64");
            assert_eq!(a.atom(ptr).as_ref(), u64_to_bytes(v).as_slice());
        }
    }

    #[rstest]
    #[case(AGG_SIG_PARENT, 10000)]
    #[case(AGG_SIG_PUZZLE, 261)]
    #[case(AGG_SIG_AMOUNT, 100_000_000_005)]
    #[case(AGG_SIG_PUZZLE_AMOUNT, 410)]
    #[case(AGG_SIG_PARENT_AMOUNT, 909)]
    #[case(AGG_SIG_PARENT_PUZZLE, 10_061_997)]
    #[case(AGG_SIG_ME, 1303)]
    fn test_make_aggsig_final_message(#[case] opcode: ConditionOpcode, #[case] coin_amount: u64) {
        use std::sync::Arc;

        use chia_protocol::Bytes32;

        use crate::conditions::SpendConditions;

        let parent_id: Vec<u8> =
            hex!("4444444444444444444444444444444444444444444444444444444444444444").into();
        let puzzle_hash: Vec<u8> =
            hex!("3333333333333333333333333333333333333333333333333333333333333333").into();
        let mut msg = b"message".to_vec();

        let mut expected_result = Vec::<u8>::new();
        expected_result.extend_from_slice(msg.as_slice());

        let coin = Coin::new(
            Bytes32::try_from(parent_id.clone()).expect("test should pass"),
            Bytes32::try_from(puzzle_hash.clone()).expect("test should pass"),
            coin_amount,
        );

        match opcode {
            AGG_SIG_PARENT => {
                expected_result.extend(parent_id.as_slice());
                expected_result.extend(TEST_CONSTANTS.agg_sig_parent_additional_data.as_slice());
            }
            AGG_SIG_PUZZLE => {
                expected_result.extend(puzzle_hash.as_slice());
                expected_result.extend(TEST_CONSTANTS.agg_sig_puzzle_additional_data.as_slice());
            }
            AGG_SIG_AMOUNT => {
                expected_result.extend(u64_to_bytes(coin_amount).as_slice());
                expected_result.extend(TEST_CONSTANTS.agg_sig_amount_additional_data.as_slice());
            }
            AGG_SIG_PUZZLE_AMOUNT => {
                expected_result.extend(puzzle_hash.as_slice());
                expected_result.extend(u64_to_bytes(coin_amount).as_slice());
                expected_result.extend(
                    TEST_CONSTANTS
                        .agg_sig_puzzle_amount_additional_data
                        .as_slice(),
                );
            }
            AGG_SIG_PARENT_AMOUNT => {
                expected_result.extend(parent_id.as_slice());
                expected_result.extend(u64_to_bytes(coin_amount).as_slice());
                expected_result.extend(
                    TEST_CONSTANTS
                        .agg_sig_parent_amount_additional_data
                        .as_slice(),
                );
            }
            AGG_SIG_PARENT_PUZZLE => {
                expected_result.extend(parent_id.as_slice());
                expected_result.extend(puzzle_hash.as_slice());
                expected_result.extend(
                    TEST_CONSTANTS
                        .agg_sig_parent_puzzle_additional_data
                        .as_slice(),
                );
            }
            AGG_SIG_ME => {
                expected_result.extend(coin.coin_id().as_slice());
                expected_result.extend(TEST_CONSTANTS.agg_sig_me_additional_data.as_slice());
            }
            _ => {}
        }
        let mut a: Allocator = make_allocator(LIMIT_HEAP);
        let spend = SpendConditions::new(
            a.new_atom(parent_id.as_slice()).expect("should pass"),
            coin_amount,
            a.new_atom(puzzle_hash.as_slice())
                .expect("test should pass"),
            Arc::new(Bytes32::try_from(coin.coin_id()).expect("test should pass")),
            0,
        );

        let spend = OwnedSpendConditions::from(&a, spend);

        make_aggsig_final_message(opcode, &mut msg, &spend, &TEST_CONSTANTS);
        assert_eq!(msg, expected_result);
    }
}
