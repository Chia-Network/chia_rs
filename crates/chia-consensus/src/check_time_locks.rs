use std::collections::HashMap;

use crate::owned_conditions::OwnedSpendBundleConditions;
use crate::validation_error::ErrorCode;
use chia_protocol::Bytes32;
use chia_protocol::CoinRecord;
use pyo3::prelude::*;
use pyo3::PyResult;
use pyo3::{pyfunction, types::PyDict};

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
            continue;
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

#[pyfunction]
#[pyo3(name = "check_time_locks")]
pub fn py_check_time_locks(
    removal_coin_records: &Bound<'_, PyDict>,
    bundle_conds: &OwnedSpendBundleConditions,
    prev_transaction_block_height: u32,
    timestamp: u64,
) -> PyResult<Option<u32>> {
    let mut removals_hashmap: HashMap<chia_protocol::BytesImpl<32>, CoinRecord> = HashMap::new();
    for (k, v) in removal_coin_records.iter() {
        let key_bytes: Bytes32 = k.extract()?;
        let coin_record: CoinRecord = v.extract()?;
        removals_hashmap.insert(key_bytes, coin_record);
    }

    let res = check_time_locks(
        &removals_hashmap,
        bundle_conds,
        prev_transaction_block_height,
        timestamp,
    );

    Ok(res.map(std::convert::Into::into))
}
