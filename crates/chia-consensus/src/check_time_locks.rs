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
) -> Result<(), ErrorCode> {
    if prev_transaction_block_height < bundle_conds.height_absolute {
        return Err(ErrorCode::AssertHeightAbsoluteFailed);
    }
    if timestamp < bundle_conds.seconds_absolute {
        return Err(ErrorCode::AssertSecondsAbsoluteFailed);
    }
    if let Some(before_height_absolute) = bundle_conds.before_height_absolute {
        if prev_transaction_block_height >= before_height_absolute {
            return Err(ErrorCode::AssertBeforeHeightAbsoluteFailed);
        }
    }
    if let Some(before_seconds_absolute) = bundle_conds.before_seconds_absolute {
        if timestamp >= before_seconds_absolute {
            return Err(ErrorCode::AssertBeforeSecondsAbsoluteFailed);
        }
    }

    for spend in &bundle_conds.spends {
        let Some(unspent) = removal_coin_records.get(&Bytes32::from(spend.coin_id)) else {
            return Err(ErrorCode::InvalidCoinId);
        };

        if let Some(birth_height) = spend.birth_height {
            if birth_height != unspent.confirmed_block_index {
                return Err(ErrorCode::AssertMyBirthHeightFailed);
            }
        }
        if let Some(birth_seconds) = spend.birth_seconds {
            if birth_seconds != unspent.timestamp {
                return Err(ErrorCode::AssertMyBirthSecondsFailed);
            }
        }
        if let Some(height_relative) = spend.height_relative {
            if prev_transaction_block_height < unspent.confirmed_block_index + height_relative {
                return Err(ErrorCode::AssertHeightRelativeFailed);
            }
        }
        if let Some(seconds_relative) = spend.seconds_relative {
            if timestamp < unspent.timestamp + seconds_relative {
                return Err(ErrorCode::AssertSecondsRelativeFailed);
            }
        }
        if let Some(before_height_relative) = spend.before_height_relative {
            if prev_transaction_block_height
                >= unspent.confirmed_block_index + before_height_relative
            {
                return Err(ErrorCode::AssertBeforeHeightRelativeFailed);
            }
        }
        if let Some(before_seconds_relative) = spend.before_seconds_relative {
            if timestamp >= unspent.timestamp + before_seconds_relative {
                return Err(ErrorCode::AssertBeforeSecondsRelativeFailed);
            }
        }
    }

    Ok(())
}

#[cfg(feature = "py-bindings")]
#[pyfunction]
#[pyo3(name = "check_time_locks")]
#[allow(clippy::needless_pass_by_value)] // pyo3 prefers pass_by_value
pub fn py_check_time_locks(
    removal_coin_records: HashMap<Bytes32, CoinRecord>,
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

    match res {
        Ok(()) => Ok(None),
        Err(ec) => Ok(Some(ec.into())),
    }
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

    #[rstest]
    #[case::height_absolute_under(
        OwnedSpendBundleConditions { height_absolute: 11, ..Default::default() },
        10,
        0,
        Err(ErrorCode::AssertHeightAbsoluteFailed)
    )]
    #[case::height_absolute_exact(
        OwnedSpendBundleConditions { height_absolute: 10, ..Default::default() },
        10,
        0,
        Ok(())
    )]
    #[case::height_absolute_over(
        OwnedSpendBundleConditions { height_absolute: 9, ..Default::default() },
        10,
        0,
        Ok(())
    )]
    #[case::seconds_absolute_under(
        OwnedSpendBundleConditions { seconds_absolute: 1001, ..Default::default() },
        0,
        1000,
        Err(ErrorCode::AssertSecondsAbsoluteFailed)
    )]
    #[case::seconds_absolute_exact(
        OwnedSpendBundleConditions { seconds_absolute: 1000, ..Default::default() },
        0,
        1000,
        Ok(())
    )]
    #[case::seconds_absolute_over(
        OwnedSpendBundleConditions { seconds_absolute: 999, ..Default::default() },
        0,
        1000,
        Ok(())
    )]
    #[case::before_height_absolute_under(
        OwnedSpendBundleConditions { before_height_absolute: Some(10), ..Default::default() },
        9,
        0,
        Ok(())
    )]
    #[case::before_height_absolute_exact(
        OwnedSpendBundleConditions { before_height_absolute: Some(10), ..Default::default() },
        10,
        0,
        Err(ErrorCode::AssertBeforeHeightAbsoluteFailed)
    )]
    #[case::before_height_absolute_over(
        OwnedSpendBundleConditions { before_height_absolute: Some(10), ..Default::default() },
        11,
        0,
        Err(ErrorCode::AssertBeforeHeightAbsoluteFailed)
    )]
    #[case::before_seconds_absolute_under(
        OwnedSpendBundleConditions { before_seconds_absolute: Some(1000), ..Default::default() },
        0,
        999,
        Ok(())
    )]
    #[case::before_seconds_absolute_exact(
        OwnedSpendBundleConditions { before_seconds_absolute: Some(1000), ..Default::default() },
        0,
        1000,
        Err(ErrorCode::AssertBeforeSecondsAbsoluteFailed)
    )]
    #[case::before_seconds_absolute_over(
        OwnedSpendBundleConditions { before_seconds_absolute: Some(1000), ..Default::default() },
        0,
        1001,
        Err(ErrorCode::AssertBeforeSecondsAbsoluteFailed)
    )]
    fn test_absolute_constraints(
        #[case] bundle: OwnedSpendBundleConditions,
        #[case] prev_height: u32,
        #[case] timestamp: u64,
        #[case] expected: Result<(), ErrorCode>,
    ) {
        let result = check_time_locks(&HashMap::new(), &bundle, prev_height, timestamp);
        assert_eq!(result, expected);
    }

    #[rstest]
    // the following cases are created with height 50, and time 1000
    #[case::height_relative_under(
        OwnedSpendConditions {
            height_relative: Some(100),
            ..Default::default()
        },
        149, // initial height 50 + 99
        2000,
        Err(ErrorCode::AssertHeightRelativeFailed)
    )]
    #[case::height_relative_exact(
        OwnedSpendConditions {
            height_relative: Some(100),
            ..Default::default()
        },
        150,  // initial height 50 + 100
        2000,
        Ok(())
    )]
    #[case::height_relative_over(
        OwnedSpendConditions {
            height_relative: Some(100),
            ..Default::default()
        },
        151,  // initial height 50 + 101
        2000,
        Ok(())
    )]
    #[case::seconds_relative_under(
        OwnedSpendConditions {
            seconds_relative: Some(1000),
            ..Default::default()
        },
        200,
        1999, // 1000 + 999
        Err(ErrorCode::AssertSecondsRelativeFailed)
    )]
    #[case::seconds_relative_exact(
        OwnedSpendConditions {
            seconds_relative: Some(1000),
            ..Default::default()
        },
        200,
        2000,  // initial 1000 + 1000
        Ok(())
    )]
    #[case::seconds_relative_over(
        OwnedSpendConditions {
            seconds_relative: Some(1000),
            ..Default::default()
        },
        200,
        2001, // initial 1000 + 1001
        Ok(())
    )]
    #[case::before_height_relative_under(
        OwnedSpendConditions {
            before_height_relative: Some(10),
            ..Default::default()
        },
        59,  // initial height 50 + 9
        1000,
        Ok(())
    )]
    #[case::before_height_relative_exact(
        OwnedSpendConditions {
            before_height_relative: Some(10),
            ..Default::default()
        },
        60,  // initial height 50 + 10
        1000,
        Err(ErrorCode::AssertBeforeHeightRelativeFailed)
    )]
    #[case::before_height_relative_over(
        OwnedSpendConditions {
            before_height_relative: Some(10),
            ..Default::default()
        },
        61,  // initial height 50 + 11
        1000,
        Err(ErrorCode::AssertBeforeHeightRelativeFailed)
    )]
    #[case::before_seconds_relative_under(
        OwnedSpendConditions {
            before_seconds_relative: Some(1000),
            ..Default::default()
        },
        100,
        1999,  // initial time 1000 + 999
        Ok(())
    )]
    #[case::before_seconds_relative_exact(
        OwnedSpendConditions {
            before_seconds_relative: Some(1000),
            ..Default::default()
        },
        100,
        2000,  // initial time 1000 + 1000
        Err(ErrorCode::AssertBeforeSecondsRelativeFailed)
    )]
    #[case::before_seconds_relative_over(
        OwnedSpendConditions {
            before_seconds_relative: Some(1000),
            ..Default::default()
        },
        100,
        2001,  // initial time 1000 + 2001
        Err(ErrorCode::AssertBeforeSecondsRelativeFailed)
    )]
    fn test_relative_constraints_failures(
        #[case] spend: OwnedSpendConditions,
        #[case] now_height: u32,
        #[case] now_timestamp: u64,
        #[case] expected: Result<(), ErrorCode>,
    ) {
        let coin_id = Bytes32::from([3u8; 32]);
        let coin_record = dummy_coin_record(50, 1000);

        let mut spend = spend;
        spend.coin_id = coin_id;

        let mut map = HashMap::new();
        map.insert(coin_id, coin_record);

        let bundle = OwnedSpendBundleConditions {
            spends: vec![spend],
            ..Default::default()
        };

        let result: Result<(), ErrorCode> =
            check_time_locks(&map, &bundle, now_height, now_timestamp);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_invalid_coin_id() {
        let coin_id = Bytes32::from([1u8; 32]);
        let spend = OwnedSpendConditions {
            coin_id,
            ..Default::default()
        };
        let bundle = OwnedSpendBundleConditions {
            spends: vec![spend],
            ..Default::default()
        };
        let result = check_time_locks(&HashMap::new(), &bundle, 0, 0);
        assert_eq!(result, Err(ErrorCode::InvalidCoinId));
    }

    #[rstest]
    fn test_all_checks_pass() {
        let coin_id = Bytes32::from([2u8; 32]);
        let coin_record = dummy_coin_record(10, 500);

        let spend = OwnedSpendConditions {
            coin_id,
            height_relative: Some(5),
            seconds_relative: Some(100),
            before_height_relative: Some(100),
            before_seconds_relative: Some(1500),
            birth_height: Some(10),
            birth_seconds: Some(500),
            ..Default::default()
        };

        let mut map = HashMap::new();
        map.insert(coin_id, coin_record);

        let bundle = OwnedSpendBundleConditions {
            spends: vec![spend],
            before_height_absolute: Some(1000),
            before_seconds_absolute: Some(2000),
            ..Default::default()
        };

        let result = check_time_locks(&map, &bundle, 20, 700);
        assert!(result.is_ok());
    }

    #[rstest]
    fn test_birth_height_and_seconds_mismatch() {
        let coin_id = Bytes32::from([4u8; 32]);
        let coin_record = dummy_coin_record(10, 500);

        let spend = OwnedSpendConditions {
            coin_id,
            birth_height: Some(11),
            birth_seconds: Some(500),
            ..Default::default()
        };

        let mut map = HashMap::new();
        map.insert(coin_id, coin_record);

        let bundle = OwnedSpendBundleConditions {
            spends: vec![spend],
            ..Default::default()
        };

        let result = check_time_locks(&map, &bundle, 100, 1000);
        assert_eq!(result, Err(ErrorCode::AssertMyBirthHeightFailed));
    }

    #[rstest]
    fn test_birth_seconds_mismatch() {
        let coin_id = Bytes32::from([5u8; 32]);
        let coin_record = dummy_coin_record(10, 500);

        let spend = OwnedSpendConditions {
            coin_id,
            birth_height: Some(10),
            birth_seconds: Some(600),
            ..Default::default()
        };

        let mut map = HashMap::new();
        map.insert(coin_id, coin_record);

        let bundle = OwnedSpendBundleConditions {
            spends: vec![spend],
            ..Default::default()
        };

        let result = check_time_locks(&map, &bundle, 100, 1000);
        assert_eq!(result, Err(ErrorCode::AssertMyBirthSecondsFailed));
    }

    #[test]
    fn test_multiple_spends_in_bundle() {
        let coin_id_1 = Bytes32::from([1u8; 32]);
        let coin_id_2 = Bytes32::from([2u8; 32]);
        let coin_id_3 = Bytes32::from([3u8; 32]);

        let coin_record_1 = dummy_coin_record(10, 500);
        let coin_record_2 = dummy_coin_record(11, 560);
        let coin_record_3 = dummy_coin_record(200, 560);

        let spend_valid = OwnedSpendConditions {
            coin_id: coin_id_1,
            birth_height: Some(10),   // this assertion is correct
            birth_seconds: Some(500), // this assertion is correct
            ..Default::default()
        };

        let spend_relative_fail = OwnedSpendConditions {
            coin_id: coin_id_2,
            height_relative: Some(50), // requires height >= 61
            birth_height: Some(11),    // this assertion is correct
            birth_seconds: Some(560),  // this assertion is correct
            ..Default::default()
        };

        let spend_birth_fail = OwnedSpendConditions {
            coin_id: coin_id_3,
            birth_height: Some(201),  // this will fail
            birth_seconds: Some(560), // this assertion is correct
            ..Default::default()
        };

        let mut map = HashMap::new();
        map.insert(coin_id_1, coin_record_1);
        map.insert(coin_id_2, coin_record_2);
        map.insert(coin_id_3, coin_record_3);

        let bundle = OwnedSpendBundleConditions {
            spends: vec![
                spend_valid.clone(),
                spend_relative_fail.clone(),
                spend_birth_fail.clone(),
            ],
            ..Default::default()
        };

        // spend_relative_fail should fail first as 59 is below required 61 height
        let result = check_time_locks(&map, &bundle, 59, 600);
        assert_eq!(result, Err(ErrorCode::AssertHeightRelativeFailed));

        let mut map = HashMap::new();
        map.insert(coin_id_1, coin_record_1);
        map.insert(coin_id_3, coin_record_3);

        let bundle = OwnedSpendBundleConditions {
            spends: vec![spend_valid, spend_birth_fail],
            ..Default::default()
        };

        // spend_birth_dail should now fail
        let result = check_time_locks(&map, &bundle, 59, 600);
        assert_eq!(result, Err(ErrorCode::AssertMyBirthHeightFailed));
    }
}
