use std::collections::HashMap;

use crate::owned_conditions::OwnedSpendBundleConditions;
use crate::validation_error::ErrorCode;
use chia_protocol::Bytes32;
use chia_protocol::CoinRecord;
#[cfg(feature = "py-bindings")]
use pyo3::pyfunction;
#[cfg(feature = "py-bindings")]
use pyo3::PyResult;

pub fn check_time_locks(
    removal_coin_records: &HashMap<Bytes32, CoinRecord>,
    bundle_conds: &OwnedSpendBundleConditions,
    prev_transaction_block_height: u32,
    timestamp: u64,
) -> Option<ErrorCode> {
    if prev_transaction_block_height < bundle_conds.height_absolute {
        return Some(ErrorCode::AssertHeightAbsoluteFailed);
    }
    if timestamp < bundle_conds.seconds_absolute {
        return Some(ErrorCode::AssertSecondsAbsoluteFailed);
    }
    if let Some(before_height_absolute) = bundle_conds.before_height_absolute {
        if prev_transaction_block_height >= before_height_absolute {
            return Some(ErrorCode::AssertBeforeHeightAbsoluteFailed);
        }
    }
    if let Some(before_seconds_absolute) = bundle_conds.before_seconds_absolute {
        if timestamp >= before_seconds_absolute {
            return Some(ErrorCode::AssertBeforeSecondsAbsoluteFailed);
        }
    }

    for spend in &bundle_conds.spends {
        let Some(unspent) = removal_coin_records.get(&Bytes32::from(spend.coin_id)) else {
            return Some(ErrorCode::InvalidCoinId);
        };

        if let Some(birth_height) = spend.birth_height {
            if birth_height != unspent.confirmed_block_index {
                return Some(ErrorCode::AssertMyBirthHeightFailed);
            }
        }
        if let Some(birth_seconds) = spend.birth_seconds {
            if birth_seconds != unspent.timestamp {
                return Some(ErrorCode::AssertMyBirthSecondsFailed);
            }
        }
        if let Some(height_relative) = spend.height_relative {
            if prev_transaction_block_height < unspent.confirmed_block_index + height_relative {
                return Some(ErrorCode::AssertHeightRelativeFailed);
            }
        }
        if let Some(seconds_relative) = spend.seconds_relative {
            if timestamp < unspent.timestamp + seconds_relative {
                return Some(ErrorCode::AssertSecondsRelativeFailed);
            }
        }
        if let Some(before_height_relative) = spend.before_height_relative {
            if prev_transaction_block_height
                >= unspent.confirmed_block_index + before_height_relative
            {
                return Some(ErrorCode::AssertBeforeHeightRelativeFailed);
            }
        }
        if let Some(before_seconds_relative) = spend.before_seconds_relative {
            if timestamp >= unspent.timestamp + before_seconds_relative {
                return Some(ErrorCode::AssertBeforeSecondsRelativeFailed);
            }
        }
    }

    None
}

#[cfg(feature = "py-bindings")]
#[pyfunction]
#[pyo3(name = "check_time_locks")]
#[allow(clippy::needless_pass_by_value)] // pyo3 prefers pass_by_value
pub fn py_check_time_locks(
    removal_coin_records: HashMap<chia_protocol::BytesImpl<32>, CoinRecord>,
    bundle_conds: &OwnedSpendBundleConditions,
    prev_transaction_block_height: u32,
    timestamp: u64,
) -> PyResult<Option<u32>> {
    let res = check_time_locks(
        &removal_coin_records,
        bundle_conds,
        prev_transaction_block_height,
        timestamp,
    );

    Ok(res.map(std::convert::Into::into))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owned_conditions::OwnedSpendConditions;
    use chia_protocol::Coin;
    use rstest::rstest;
    use std::collections::HashMap;

    fn dummy_coin_record(confirmed_block_index: u32, timestamp: u64) -> CoinRecord {
        CoinRecord {
            coin: Coin {
                parent_coin_info: Bytes32::default(),
                puzzle_hash: Bytes32::default(),
                amount: 1,
            },
            confirmed_block_index,
            spent_block_index: 0,
            coinbase: false,
            timestamp,
        }
    }

    fn dummy_spend_conditions(
        coin_id: Bytes32,
        height_relative: Option<u32>,
        seconds_relative: Option<u64>,
        before_height_relative: Option<u32>,
        before_seconds_relative: Option<u64>,
        birth_height: Option<u32>,
        birth_seconds: Option<u64>,
    ) -> OwnedSpendConditions {
        OwnedSpendConditions {
            coin_id,
            parent_id: Bytes32::default(),
            puzzle_hash: Bytes32::default(),
            coin_amount: 1,
            height_relative,
            seconds_relative,
            before_height_relative,
            before_seconds_relative,
            birth_height,
            birth_seconds,
            create_coin: vec![],
            agg_sig_me: vec![],
            agg_sig_parent: vec![],
            agg_sig_puzzle: vec![],
            agg_sig_amount: vec![],
            agg_sig_puzzle_amount: vec![],
            agg_sig_parent_amount: vec![],
            agg_sig_parent_puzzle: vec![],
            flags: 0,
            execution_cost: 0,
            condition_cost: 0,
        }
    }

    fn dummy_bundle(spends: Vec<OwnedSpendConditions>) -> OwnedSpendBundleConditions {
        OwnedSpendBundleConditions {
            spends,
            reserve_fee: 0,
            height_absolute: 0,
            seconds_absolute: 0,
            before_height_absolute: None,
            before_seconds_absolute: None,
            agg_sig_unsafe: vec![],
            cost: 0,
            removal_amount: 0,
            addition_amount: 0,
            validated_signature: true,
            execution_cost: 0,
            condition_cost: 0,
        }
    }

    #[rstest]
    #[case(0, 0, 10, 1000, None, None, None)]
    #[case(
        20,
        0,
        10,
        1000,
        Some(ErrorCode::AssertHeightAbsoluteFailed),
        None,
        None
    )]
    #[case(
        0,
        2000,
        10,
        1000,
        Some(ErrorCode::AssertSecondsAbsoluteFailed),
        None,
        None
    )]
    #[case(
        0,
        0,
        10,
        1000,
        Some(ErrorCode::AssertBeforeHeightAbsoluteFailed),
        Some(5),
        None
    )]
    #[case(
        0,
        0,
        10,
        2000,
        Some(ErrorCode::AssertBeforeSecondsAbsoluteFailed),
        None,
        Some(1500)
    )]
    fn test_absolute_constraints(
        #[case] height_absolute: u32,
        #[case] seconds_absolute: u64,
        #[case] prev_height: u32,
        #[case] timestamp: u64,
        #[case] expected: Option<ErrorCode>,
        #[case] before_height_absolute: Option<u32>,
        #[case] before_seconds_absolute: Option<u64>,
    ) {
        let bundle = OwnedSpendBundleConditions {
            height_absolute,
            seconds_absolute,
            before_height_absolute,
            before_seconds_absolute,
            ..dummy_bundle(vec![])
        };

        // no OwnedSpendConditions in vec so only check absolutes
        let result = check_time_locks(&HashMap::new(), &bundle, prev_height, timestamp);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_invalid_coin_id() {
        let coin_id = Bytes32::from([1u8; 32]);
        let spend = dummy_spend_conditions(coin_id, None, None, None, None, None, None);
        let bundle = dummy_bundle(vec![spend]);
        // coin_id not in HashMap
        let result = check_time_locks(&HashMap::new(), &bundle, 0, 0);
        assert_eq!(result, Some(ErrorCode::InvalidCoinId));
    }

    #[rstest]
    fn test_all_checks_pass() {
        let coin_id = Bytes32::from([2u8; 32]);
        let coin_record = dummy_coin_record(10, 500);

        let spend = dummy_spend_conditions(
            coin_id,
            Some(5),
            Some(100),
            Some(100),
            Some(1500),
            Some(10),
            Some(500),
        );

        let mut map = HashMap::new();
        map.insert(coin_id, coin_record);

        let bundle = OwnedSpendBundleConditions {
            height_absolute: 0,
            seconds_absolute: 0,
            before_height_absolute: Some(1000),
            before_seconds_absolute: Some(2000),
            ..dummy_bundle(vec![spend])
        };

        let result = check_time_locks(&map, &bundle, 20, 700);
        assert_eq!(result, None);
    }

    #[rstest]
    fn test_relative_constraints_failures() {
        let coin_id = Bytes32::from([3u8; 32]);
        let coin_record = dummy_coin_record(50, 1000);

        let spend = dummy_spend_conditions(
            coin_id,
            Some(100),  // height_relative (requires height >= 150)
            Some(1000), // seconds_relative (requires timestamp >= 2000) -- will fail
            Some(10),   // before_height_relative (fails if height >= 60)
            Some(1000), // before_seconds_relative (fails if timestamp >= 2000)
            Some(50),
            Some(1000),
        );

        let mut map = HashMap::new();
        map.insert(coin_id, coin_record);

        let bundle = dummy_bundle(vec![spend]);

        let result = check_time_locks(&map, &bundle, 160, 1600);
        assert_eq!(result, Some(ErrorCode::AssertSecondsRelativeFailed));
    }

    #[rstest]
    fn test_birth_height_and_seconds_mismatch() {
        let coin_id = Bytes32::from([4u8; 32]);
        let coin_record = dummy_coin_record(10, 500);

        let spend = dummy_spend_conditions(
            coin_id,
            None,
            None,
            None,
            None,
            Some(11), // wrong birth_height
            Some(500),
        );

        let mut map = HashMap::new();
        map.insert(coin_id, coin_record);

        let bundle = dummy_bundle(vec![spend]);
        let result = check_time_locks(&map, &bundle, 100, 1000);
        assert_eq!(result, Some(ErrorCode::AssertMyBirthHeightFailed));
    }

    #[rstest]
    fn test_birth_seconds_mismatch() {
        let coin_id = Bytes32::from([5u8; 32]);
        let coin_record = dummy_coin_record(10, 500);

        let spend = dummy_spend_conditions(
            coin_id,
            None,
            None,
            None,
            None,
            Some(10),
            Some(600), // wrong birth_seconds
        );

        let mut map = HashMap::new();
        map.insert(coin_id, coin_record);

        let bundle = dummy_bundle(vec![spend]);
        let result = check_time_locks(&map, &bundle, 100, 1000);
        assert_eq!(result, Some(ErrorCode::AssertMyBirthSecondsFailed));
    }
}
