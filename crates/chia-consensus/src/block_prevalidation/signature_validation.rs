use chia_protocol::{Bytes, Bytes32, Bytes48, Coin};
use sha2::{digest::FixedOutput, Digest, Sha256};

use crate::{
    gen::{
        opcodes::{
            ConditionOpcode, AGG_SIG_AMOUNT, AGG_SIG_ME, AGG_SIG_PARENT, AGG_SIG_PARENT_AMOUNT,
            AGG_SIG_PARENT_PUZZLE, AGG_SIG_PUZZLE, AGG_SIG_PUZZLE_AMOUNT,
        },
        owned_conditions::OwnedSpendBundleConditions,
    },
    int_to_bytes::u64_to_bytes,
};

pub fn pkm_pairs(
    conditions: &OwnedSpendBundleConditions,
    agg_sig_data: Bytes32,
) -> Option<Vec<(Bytes48, Bytes)>> {
    let mut pairs = Vec::new();

    for (pk, msg) in conditions.agg_sig_unsafe.clone() {
        for condition_code in agg_sig_non_unsafe() {
            if msg.ends_with(&agg_sig_additional_data(agg_sig_data, *condition_code)) {
                return None;
            }
        }
        pairs.push((pk, msg));
    }

    for spend in conditions.spends.iter() {
        let opcode_items = [
            (AGG_SIG_PARENT, spend.agg_sig_parent.clone()),
            (AGG_SIG_PUZZLE, spend.agg_sig_puzzle.clone()),
            (AGG_SIG_AMOUNT, spend.agg_sig_amount.clone()),
            (AGG_SIG_PUZZLE_AMOUNT, spend.agg_sig_puzzle_amount.clone()),
            (AGG_SIG_PARENT_AMOUNT, spend.agg_sig_parent_amount.clone()),
            (AGG_SIG_PARENT_PUZZLE, spend.agg_sig_parent_puzzle.clone()),
            (AGG_SIG_ME, spend.agg_sig_me.clone()),
        ];

        for (opcode, items) in opcode_items {
            for (pk, msg) in items {
                pairs.push((
                    pk,
                    make_final_aggsig_message(
                        opcode,
                        msg,
                        spend.to_coin(),
                        agg_sig_additional_data(agg_sig_data, opcode),
                    ),
                ));
            }
        }
    }

    Some(pairs)
}

/// # Panics
///
/// Will panic unless called with one of the following opcodes:
/// * `AGG_SIG_PARENT`
/// * `AGG_SIG_PUZZLE`
/// * `AGG_SIG_AMOUNT`
/// * `AGG_SIG_PUZZLE_AMOUNT`
/// * `AGG_SIG_PARENT_AMOUNT`
/// * `AGG_SIG_PARENT_PUZZLE`
/// * `AGG_SIG_ME`
fn make_final_aggsig_message(
    opcode: ConditionOpcode,
    msg: Bytes,
    coin: Coin,
    agg_sig_additional_data: Bytes32,
) -> Bytes {
    let mut bytes = msg.into_inner();

    match opcode {
        AGG_SIG_PARENT => bytes.extend(coin.parent_coin_info.as_ref()),
        AGG_SIG_PUZZLE => bytes.extend(coin.puzzle_hash.as_ref()),
        AGG_SIG_AMOUNT => bytes.extend(u64_to_bytes(coin.amount)),
        AGG_SIG_PUZZLE_AMOUNT => {
            bytes.extend(coin.puzzle_hash.as_ref());
            bytes.extend(u64_to_bytes(coin.amount));
        }
        AGG_SIG_PARENT_AMOUNT => {
            bytes.extend(coin.parent_coin_info.as_ref());
            bytes.extend(u64_to_bytes(coin.amount));
        }
        AGG_SIG_PARENT_PUZZLE => {
            bytes.extend(coin.parent_coin_info.as_ref());
            bytes.extend(coin.puzzle_hash.as_ref());
        }
        AGG_SIG_ME => bytes.extend(coin.coin_id().as_ref()),
        _ => panic!("cannot make final aggsig message for opcode {:?}", opcode),
    }

    bytes.extend(agg_sig_additional_data.as_ref());

    Bytes::new(bytes)
}

fn agg_sig_additional_data(agg_sig_data: Bytes32, condition_code: ConditionOpcode) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(agg_sig_data);
    hasher.update([condition_code as u8]);
    Bytes32::new(hasher.finalize_fixed().into())
}

fn agg_sig_non_unsafe() -> &'static [ConditionOpcode] {
    &[
        AGG_SIG_PARENT,
        AGG_SIG_PUZZLE,
        AGG_SIG_AMOUNT,
        AGG_SIG_PUZZLE_AMOUNT,
        AGG_SIG_PARENT_AMOUNT,
        AGG_SIG_PARENT_PUZZLE,
        AGG_SIG_ME,
    ]
}
