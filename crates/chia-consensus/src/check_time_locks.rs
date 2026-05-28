use std::collections::HashMap;

use crate::owned_conditions::OwnedSpendBundleConditions;
use crate::validation_error::{ErrorCode, ValidationErr};
use chia_protocol::Bytes32;
use chia_protocol::CoinRecord;
#[cfg(feature = "py-bindings")]
use pyo3::PyResult;
#[cfg(feature = "py-bindings")]
use pyo3::pyfunction;

pub fn check_time_locks(
    removal_coin_records: &HashMap<Bytes32, CoinRecord>,
    bundle_conds: &OwnedSpendBundleConditions,
    prev_transaction_block_height: u32,
    timestamp: u64,
    nowrap: bool,
) -> Result<(), ValidationErr> {
    if prev_transaction_block_height < bundle_conds.height_absolute {
        return Err(ValidationErr::Err(ErrorCode::AssertHeightAbsoluteFailed));
    }
    if timestamp < bundle_conds.seconds_absolute {
        return Err(ValidationErr::Err(ErrorCode::AssertSecondsAbsoluteFailed));
    }
    if let Some(before_height_absolute) = bundle_conds.before_height_absolute {
        if prev_transaction_block_height >= before_height_absolute {
            return Err(ValidationErr::Err(
                ErrorCode::AssertBeforeHeightAbsoluteFailed,
            ));
        }
    }
    if let Some(before_seconds_absolute) = bundle_conds.before_seconds_absolute {
        if timestamp >= before_seconds_absolute {
            return Err(ValidationErr::Err(
                ErrorCode::AssertBeforeSecondsAbsoluteFailed,
            ));
        }
    }

    for spend in &bundle_conds.spends {
        let Some(unspent) = removal_coin_records.get(&Bytes32::from(spend.coin_id)) else {
            return Err(ValidationErr::Err(ErrorCode::InvalidCoinId));
        };

        if let Some(birth_height) = spend.birth_height {
            if birth_height != unspent.confirmed_block_index {
                return Err(ValidationErr::Err(ErrorCode::AssertMyBirthHeightFailed));
            }
        }
        if let Some(birth_seconds) = spend.birth_seconds {
            if birth_seconds != unspent.timestamp {
                return Err(ValidationErr::Err(ErrorCode::AssertMyBirthSecondsFailed));
            }
        }
        if let Some(height_relative) = spend.height_relative {
            if nowrap {
                if prev_transaction_block_height
                    < unspent
                        .confirmed_block_index
                        .saturating_add(height_relative)
                {
                    return Err(ValidationErr::Err(ErrorCode::AssertHeightRelativeFailed));
                }
            } else if prev_transaction_block_height
                < unspent.confirmed_block_index.wrapping_add(height_relative)
            {
                return Err(ValidationErr::Err(ErrorCode::AssertHeightRelativeFailed));
            }
        }
        if let Some(seconds_relative) = spend.seconds_relative {
            if nowrap {
                if timestamp < unspent.timestamp.saturating_add(seconds_relative) {
                    return Err(ValidationErr::Err(ErrorCode::AssertSecondsRelativeFailed));
                }
            } else if timestamp < unspent.timestamp.wrapping_add(seconds_relative) {
                return Err(ValidationErr::Err(ErrorCode::AssertSecondsRelativeFailed));
            }
        }
        if let Some(before_height_relative) = spend.before_height_relative {
            if nowrap {
                if prev_transaction_block_height
                    >= unspent
                        .confirmed_block_index
                        .saturating_add(before_height_relative)
                {
                    return Err(ValidationErr::Err(
                        ErrorCode::AssertBeforeHeightRelativeFailed,
                    ));
                }
            } else if prev_transaction_block_height
                >= unspent
                    .confirmed_block_index
                    .wrapping_add(before_height_relative)
            {
                return Err(ValidationErr::Err(
                    ErrorCode::AssertBeforeHeightRelativeFailed,
                ));
            }
        }
        if let Some(before_seconds_relative) = spend.before_seconds_relative {
            if nowrap {
                if timestamp >= unspent.timestamp.saturating_add(before_seconds_relative) {
                    return Err(ValidationErr::Err(
                        ErrorCode::AssertBeforeSecondsRelativeFailed,
                    ));
                }
            } else if timestamp >= unspent.timestamp.wrapping_add(before_seconds_relative) {
                return Err(ValidationErr::Err(
                    ErrorCode::AssertBeforeSecondsRelativeFailed,
                ));
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
    nowrap: bool,
) -> PyResult<Option<u32>> {
    let res = check_time_locks(
        &removal_coin_records,
        bundle_conds,
        prev_transaction_block_height,
        timestamp,
        nowrap,
    );

    match res {
        Ok(()) => Ok(None),
        Err(ec) => Ok(Some(ec.error_code().into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::owned_conditions::OwnedSpendConditions;
    use crate::validation_error::ValidationErr;
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
        Err(ValidationErr::Err(ErrorCode::AssertHeightAbsoluteFailed))
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
        Err(ValidationErr::Err(ErrorCode::AssertSecondsAbsoluteFailed))
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
        Err(ValidationErr::Err(ErrorCode::AssertBeforeHeightAbsoluteFailed))
    )]
    #[case::before_height_absolute_over(
        OwnedSpendBundleConditions { before_height_absolute: Some(10), ..Default::default() },
        11,
        0,
        Err(ValidationErr::Err(ErrorCode::AssertBeforeHeightAbsoluteFailed))
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
        Err(ValidationErr::Err(ErrorCode::AssertBeforeSecondsAbsoluteFailed))
    )]
    #[case::before_seconds_absolute_over(
        OwnedSpendBundleConditions { before_seconds_absolute: Some(1000), ..Default::default() },
        0,
        1001,
        Err(ValidationErr::Err(ErrorCode::AssertBeforeSecondsAbsoluteFailed))
    )]
    fn test_absolute_constraints(
        #[case] bundle: OwnedSpendBundleConditions,
        #[case] prev_height: u32,
        #[case] timestamp: u64,
        #[case] expected: Result<(), ValidationErr>,
        #[values(true, false)] nowrap: bool,
    ) {
        let result = check_time_locks(&HashMap::new(), &bundle, prev_height, timestamp, nowrap);
        assert_eq!(result, expected);
    }

    type Osc = OwnedSpendConditions;

    #[rstest]
    // Coin confirmed at height 100, timestamp 1000.
    // Checked at height 200, timestamp 2000.
    // Each case: (spend, expected_nowrap, expected_no_nowrap)
    //
    // height_relative check: prev_height < confirmed + height_relative -> Err
    // 200 < 100 + 101 = 201 -> Err (both agree, no overflow)
    #[case::height_relative_under(
        Osc { height_relative: Some(101), ..Default::default() },
        Err(ValidationErr::Err(ErrorCode::AssertHeightRelativeFailed)),
        Err(ValidationErr::Err(ErrorCode::AssertHeightRelativeFailed)),
    )]
    // 200 < 100 + 100 = 200 -> Ok (both agree, no overflow)
    #[case::height_relative_exact(
        Osc { height_relative: Some(100), ..Default::default() },
        Ok(()),
        Ok(()),
    )]
    // 200 < 100 + 99 = 199 -> Ok (both agree, no overflow)
    #[case::height_relative_over(
        Osc { height_relative: Some(99), ..Default::default() },
        Ok(()),
        Ok(()),
    )]
    // 200 < 100 + u32::MAX -> Err with nowrap (saturates), Ok without (wraps)
    #[case::height_relative_wrap(
        Osc { height_relative: Some(0xffff_ffff), ..Default::default() },
        Err(ValidationErr::Err(ErrorCode::AssertHeightRelativeFailed)),
        Ok(()),
    )]
    // seconds_relative check: timestamp < coin_time + seconds_relative -> Err
    // 2000 < 1000 + 1001 = 2001 -> Err (both agree, no overflow)
    #[case::seconds_relative_under(
        Osc { seconds_relative: Some(1001), ..Default::default() },
        Err(ValidationErr::Err(ErrorCode::AssertSecondsRelativeFailed)),
        Err(ValidationErr::Err(ErrorCode::AssertSecondsRelativeFailed)),
    )]
    // 2000 < 1000 + 1000 = 2000 -> Ok (both agree, no overflow)
    #[case::seconds_relative_exact(
        Osc { seconds_relative: Some(1000), ..Default::default() },
        Ok(()),
        Ok(()),
    )]
    // 2000 < 1000 + 999 = 1999 -> Ok (both agree, no overflow)
    #[case::seconds_relative_over(
        Osc { seconds_relative: Some(999), ..Default::default() },
        Ok(()),
        Ok(()),
    )]
    // 2000 < 1000 + u64::MAX -> Err with nowrap (saturates), Ok without (wraps)
    #[case::seconds_relative_wrap(
        Osc { seconds_relative: Some(0xffff_ffff_ffff_ffff), ..Default::default() },
        Err(ValidationErr::Err(ErrorCode::AssertSecondsRelativeFailed)),
        Ok(()),
    )]
    // before_height_relative check: prev_height >= confirmed + before_height_relative -> Err
    // 200 >= 100 + 101 = 201 -> Ok (both agree, no overflow)
    #[case::before_height_relative_under(
        Osc { before_height_relative: Some(101), ..Default::default() },
        Ok(()),
        Ok(()),
    )]
    // 200 >= 100 + 100 = 200 -> Err (both agree, no overflow)
    #[case::before_height_relative_exact(
        Osc { before_height_relative: Some(100), ..Default::default() },
        Err(ValidationErr::Err(ErrorCode::AssertBeforeHeightRelativeFailed)),
        Err(ValidationErr::Err(ErrorCode::AssertBeforeHeightRelativeFailed)),
    )]
    // 200 >= 100 + 99 = 199 -> Err (both agree, no overflow)
    #[case::before_height_relative_over(
        Osc { before_height_relative: Some(99), ..Default::default() },
        Err(ValidationErr::Err(ErrorCode::AssertBeforeHeightRelativeFailed)),
        Err(ValidationErr::Err(ErrorCode::AssertBeforeHeightRelativeFailed)),
    )]
    // 200 >= 100 + 0xffff_ffff -> Ok with nowrap (saturates), Err without (wraps)
    #[case::before_height_relative_wrap(
        Osc { before_height_relative: Some(0xffff_ffff), ..Default::default() },
        Ok(()),
        Err(ValidationErr::Err(ErrorCode::AssertBeforeHeightRelativeFailed)),
    )]
    // before_seconds_relative check: timestamp >= coin_time + before_seconds_relative -> Err
    // 2000 >= 1000 + 1001 = 2001 -> Ok (both agree, no overflow)
    #[case::before_seconds_relative_under(
        Osc { before_seconds_relative: Some(1001), ..Default::default() },
        Ok(()),
        Ok(()),
    )]
    // 2000 >= 1000 + 1000 = 2000 -> Err (both agree, no overflow)
    #[case::before_seconds_relative_exact(
        Osc { before_seconds_relative: Some(1000), ..Default::default() },
        Err(ValidationErr::Err(ErrorCode::AssertBeforeSecondsRelativeFailed)),
        Err(ValidationErr::Err(ErrorCode::AssertBeforeSecondsRelativeFailed)),
    )]
    // 2000 >= 1000 + 999 = 1999 -> Err (both agree, no overflow)
    #[case::before_seconds_relative_over(
        Osc { before_seconds_relative: Some(999), ..Default::default() },
        Err(ValidationErr::Err(ErrorCode::AssertBeforeSecondsRelativeFailed)),
        Err(ValidationErr::Err(ErrorCode::AssertBeforeSecondsRelativeFailed)),
    )]
    // 2000 >= 1000 + u64::MAX -> Ok with nowrap (saturates), Err without (wraps)
    #[case::before_seconds_relative_wrap(
        Osc { before_seconds_relative: Some(0xffff_ffff_ffff_ffff), ..Default::default() },
        Ok(()),
        Err(ValidationErr::Err(ErrorCode::AssertBeforeSecondsRelativeFailed)),
    )]
    fn test_relative_constraints(
        #[case] spend: OwnedSpendConditions,
        #[case] expected_nowrap: Result<(), ValidationErr>,
        #[case] expected_no_nowrap: Result<(), ValidationErr>,
        #[values(true, false)] nowrap: bool,
    ) {
        let expected = if nowrap {
            expected_nowrap
        } else {
            expected_no_nowrap
        };
        let now_height = 200_u32;
        let now_timestamp = 2000_u64;

        let coin_id = Bytes32::from([3u8; 32]);
        let coin_record = dummy_coin_record(100, 1000);

        let mut spend = spend;
        spend.coin_id = coin_id;

        let mut map = HashMap::new();
        map.insert(coin_id, coin_record);

        let bundle = OwnedSpendBundleConditions {
            spends: vec![spend],
            ..Default::default()
        };

        let result: Result<(), ValidationErr> =
            check_time_locks(&map, &bundle, now_height, now_timestamp, nowrap);
        assert_eq!(result, expected);
    }

    #[rstest]
    fn test_invalid_coin_id(#[values(true, false)] nowrap: bool) {
        let coin_id = Bytes32::from([1u8; 32]);
        let spend = OwnedSpendConditions {
            coin_id,
            ..Default::default()
        };
        let bundle = OwnedSpendBundleConditions {
            spends: vec![spend],
            ..Default::default()
        };
        let result = check_time_locks(&HashMap::new(), &bundle, 0, 0, nowrap);
        assert_eq!(result, Err(ValidationErr::Err(ErrorCode::InvalidCoinId)));
    }

    #[rstest]
    fn test_all_checks_pass(#[values(true, false)] nowrap: bool) {
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

        let result = check_time_locks(&map, &bundle, 20, 700, nowrap);
        assert!(result.is_ok());
    }

    #[rstest]
    fn test_birth_height_and_seconds_mismatch(#[values(true, false)] nowrap: bool) {
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

        let result = check_time_locks(&map, &bundle, 100, 1000, nowrap);
        assert_eq!(
            result,
            Err(ValidationErr::Err(ErrorCode::AssertMyBirthHeightFailed))
        );
    }

    #[rstest]
    fn test_birth_seconds_mismatch(#[values(true, false)] nowrap: bool) {
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

        let result = check_time_locks(&map, &bundle, 100, 1000, nowrap);
        assert_eq!(
            result,
            Err(ValidationErr::Err(ErrorCode::AssertMyBirthSecondsFailed))
        );
    }

    #[rstest]
    fn test_multiple_spends_in_bundle(#[values(true, false)] nowrap: bool) {
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
        let result = check_time_locks(&map, &bundle, 59, 600, nowrap);
        assert_eq!(
            result,
            Err(ValidationErr::Err(ErrorCode::AssertHeightRelativeFailed))
        );

        let mut map = HashMap::new();
        map.insert(coin_id_1, coin_record_1);
        map.insert(coin_id_3, coin_record_3);

        let bundle = OwnedSpendBundleConditions {
            spends: vec![spend_valid, spend_birth_fail],
            ..Default::default()
        };

        // spend_birth_fail should now fail
        let result = check_time_locks(&map, &bundle, 59, 600, nowrap);
        assert_eq!(
            result,
            Err(ValidationErr::Err(ErrorCode::AssertMyBirthHeightFailed))
        );
    }
}
