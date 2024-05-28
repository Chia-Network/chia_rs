use std::collections::HashMap;
use crate::gen::owned_conditions::{OwnedSpendBundleConditions, OwnedSpend};
use chia_bls::PublicKey;
use chia_protocol::Coin;
use chia_protocol::Bytes;
use crate::gen::validation_error::ErrorCode;
use crate::gen::opcodes::{AGG_SIG_AMOUNT, AGG_SIG_PARENT, AGG_SIG_PARENT_PUZZLE,
    AGG_SIG_PUZZLE_AMOUNT, AGG_SIG_ME, AGG_SIG_UNSAFE, AGG_SIG_PUZZLE, AGG_SIG_PARENT_AMOUNT, ConditionOpcode};
use crate::gen::conditions::Spend;

pub fn pkm_pairs(conditions: OwnedSpendBundleConditions, additional_data: &[u8]) -> Result<(Vec<PublicKey>, Vec<Vec<u8>>), ErrorCode> {
    let mut pks = Vec::<PublicKey>::new();
    let mut msgs = Vec::<Vec<u8>>::new();
    let disallowed = agg_sig_additional_data(additional_data);
    for (pk, msg) in conditions.agg_sig_unsafe {
        pks.push(pk);
        msgs.push(msg.as_slice().to_vec());
        for (_, disallowed_val) in disallowed.into_iter() {
            if msg.ends_with(disallowed_val.as_slice()) {
                return Err(ErrorCode::InvalidCondition)
            }
        }
    }
    for spend in conditions.spends {
        let condition_items_pairs = [
            (AGG_SIG_PARENT, spend.agg_sig_parent),
            (AGG_SIG_PUZZLE, spend.agg_sig_puzzle),
            (AGG_SIG_AMOUNT, spend.agg_sig_amount),
            (AGG_SIG_PUZZLE_AMOUNT, spend.agg_sig_puzzle_amount),
            (AGG_SIG_PARENT_AMOUNT, spend.agg_sig_parent_amount),
            (AGG_SIG_PARENT_PUZZLE, spend.agg_sig_parent_puzzle),
            (AGG_SIG_ME, spend.agg_sig_me),
        ];
        for (condition, items) in condition_items_pairs {
            for (pk, msg) in items {
                pks.push(pk);
                msgs.push(make_aggsig_final_message(condition, msg.as_slice().to_vec(), spend, disallowed));
            }
        }
    }
    Ok((pks, msgs))
}

fn make_aggsig_final_message(
    opcode: ConditionOpcode,
    msg: Vec<u8>,
    spend: OwnedSpend,
    agg_sig_additional_data: HashMap<ConditionOpcode, Vec<u8>>,
) -> Vec<u8> {
    let addendum: Vec<u8> = match opcode {
        AGG_SIG_PARENT => spend.parent_id.as_slice().to_vec(),
        AGG_SIG_PUZZLE => spend.puzzle_hash.as_slice().to_vec(),
        AGG_SIG_AMOUNT => u64_to_bytes(spend.coin_amount).as_slice().to_vec(),
        AGG_SIG_PUZZLE_AMOUNT => {
            [spend.parent_id.as_slice(), u64_to_bytes(spend.coin_amount).as_slice()].concat()
        }
        AGG_SIG_PARENT_AMOUNT => {
            [spend.parent_id.as_slice(), u64_to_bytes(spend.coin_amount).as_slice()].concat()
        }
        AGG_SIG_PARENT_PUZZLE => {
            [spend.parent_id.as_slice(), spend.puzzle_hash.as_slice()].concat()
        }
        AGG_SIG_ME => {
            let coin: Coin = Coin::new(
                spend.parent_id,
                spend.puzzle_hash,
                spend.coin_amount as u64,
            );
            coin.coin_id().as_slice().to_vec()
        },
    };

    let mut result = msg.to_vec();
    result.extend(addendum);
    if let Some(additional_data) = agg_sig_additional_data.get(&opcode) {
        result.extend(additional_data.clone());
    }

    result
}

fn u64_to_bytes(val: u64) -> Bytes {
    let amount_bytes: [u8; 8] = val.to_be_bytes();
        if val >= 0x8000000000000000_u64 {
            let mut ret = Vec::<u8>::new();
            ret.push(0_u8);
            ret.extend(amount_bytes);
            Bytes::new(ret)
        } else {
            let start = match val {
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
            Bytes::new(amount_bytes[start..].to_vec())
        }
}

fn agg_sig_additional_data(agg_sig_data: &[u8]) -> HashMap<ConditionOpcode, Vec<u8>> {
    let mut ret: HashMap<ConditionOpcode, &[u8]> = HashMap::new();
    for code in [
        AGG_SIG_PARENT,
        AGG_SIG_PUZZLE,
        AGG_SIG_AMOUNT,
        AGG_SIG_PUZZLE_AMOUNT,
        AGG_SIG_PARENT_AMOUNT,
        AGG_SIG_PARENT_PUZZLE,
    ] {
        ret.insert(code, std_hash(&(agg_sig_data.clone() + &[code as u8])));
    }
    ret.insert(AGG_SIG_ME, agg_sig_data);

    ret
}