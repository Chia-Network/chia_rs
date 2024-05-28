use std::collections::HashMap;
use crate::gen::owned_conditions::{OwnedSpendBundleConditions, OwnedSpend};
use chia_bls::PublicKey;
use chia_protocol::Coin;
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
    let coin: Coin = Coin::new(
        spend.parent_id.clone(),
        spend.puzzle_hash.clone(),
        spend.coin_amount as u64,
    );

    let addendum = match opcode {
        AGG_SIG_PARENT => coin.parent_coin_info.clone(),
        AGG_SIG_PUZZLE => coin.puzzle_hash.clone(),
        AGG_SIG_AMOUNT => int_to_bytes(coin.amount),
        AGG_SIG_PUZZLE_AMOUNT => {
            let mut data = coin.puzzle_hash.clone();
            data.extend(int_to_bytes(coin.amount));
            data
        }
        AGG_SIG_PARENT_AMOUNT => {
            let mut data = coin.parent_coin_info.clone();
            data.extend(int_to_bytes(coin.amount));
            data
        }
        AGG_SIG_PARENT_PUZZLE => {
            let mut data = coin.parent_coin_info.clone();
            data.extend(coin.puzzle_hash.clone());
            data
        }
        AGG_SIG_ME => coin_name(&coin),
    };

    let mut result = msg.to_vec();
    result.extend(addendum);
    if let Some(additional_data) = agg_sig_additional_data.get(&opcode) {
        result.extend(additional_data.clone());
    }

    result.as_slice()
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