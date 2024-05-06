use crate::gen::SpendBundleConditions;
use crate::gen::conditions::Condition;
use chia_bls::PublicKey;
use crate::gen::opcodes::{AGG_SIG_AMOUNT, AGG_SIG_PARENT, AGG_SIG_PARENT_PUZZLE, AGG_SIG_PUZZLE_AMOUNT,
    AGG_SIG_PUZZLE_AMOUNT, AGG_SIG_ME, AGG_SIG_UNSAFE, AGG_SIG_PUZZLE, AGG_SIG_PARENT_AMOUNT};

pub fn pkm_pairs(conditions: OwnedSpendBundleConditions, additional_data: &[u8]) -> (Vec<PublicKey>, Vec<&[u8]>) {
    let pks = Vec<PublicKey>::new();
    let msg = Vec<V&[u8]>::new();
    for (pk, msg) in conditions.agg_sig_unsafe {
        pks.push(pk);
        msgs.push(msg);
    }
    for spend in conditions.spends {
        let condition_items_pairs = [
            (ConditionOpcode.AGG_SIG_PARENT, spend.agg_sig_parent),
            (ConditionOpcode.AGG_SIG_PUZZLE, spend.agg_sig_puzzle),
            (ConditionOpcode.AGG_SIG_AMOUNT, spend.agg_sig_amount),
            (ConditionOpcode.AGG_SIG_PUZZLE_AMOUNT, spend.agg_sig_puzzle_amount),
            (ConditionOpcode.AGG_SIG_PARENT_AMOUNT, spend.agg_sig_parent_amount),
            (ConditionOpcode.AGG_SIG_PARENT_PUZZLE, spend.agg_sig_parent_puzzle),
            (ConditionOpcode.AGG_SIG_ME, spend.agg_sig_me),
        ];
        for (condition, items) in condition_items_pairs {
            for (pk, msg) in items {
                pks.push(pk);
                msgs.push(make_aggsig_final_message(condition, &msg, spend, data));
            }
        }
    }
    return (pks, msgs)
}

use std::collections::HashMap;

// Assuming the necessary types and functions are defined elsewhere

fn make_aggsig_final_message(
    opcode: ConditionOpcode,
    msg: Vec<u8>,
    spend: &dyn Into<Coin>,
    agg_sig_additional_data: HashMap<ConditionOpcode, Vec<u8>>,
) -> Vec<u8> {
    let coin: Coin;
    if let Some(coin_spend) = spend.into().as_any().downcast_ref::<Coin>() {
        coin = coin_spend.clone();
    } else if let Some(spend) = spend.into().as_any().downcast_ref::<Spend>() {
        coin = Coin::new(
            spend.parent_id.clone(),
            spend.puzzle_hash.clone(),
            spend.coin_amount as u64,
        );
    } else {
        panic!("Expected Coin or Spend, got {:?}", spend); // You can choose to handle this differently
    }

    let mut coin_to_addendum_f_lookup: HashMap<ConditionOpcode, Box<dyn Fn(&Coin) -> Vec<u8>>> = HashMap::new();
    coin_to_addendum_f_lookup.insert(ConditionOpcode::AggSigParent, Box::new(|coin| coin.parent_coin_info.clone()));
    coin_to_addendum_f_lookup.insert(ConditionOpcode::AggSigPuzzle, Box::new(|coin| coin.puzzle_hash.clone()));
    coin_to_addendum_f_lookup.insert(ConditionOpcode::AggSigAmount, Box::new(|coin| int_to_bytes(coin.amount)));
    coin_to_addendum_f_lookup.insert(ConditionOpcode::AggSigPuzzleAmount, Box::new(|coin| coin.puzzle_hash.clone() + &int_to_bytes(coin.amount)));
    coin_to_addendum_f_lookup.insert(ConditionOpcode::AggSigParentAmount, Box::new(|coin| coin.parent_coin_info.clone() + &int_to_bytes(coin.amount)));
    coin_to_addendum_f_lookup.insert(ConditionOpcode::AggSigParentPuzzle, Box::new(|coin| coin.parent_coin_info.clone() + &coin.puzzle_hash.clone()));
    coin_to_addendum_f_lookup.insert(ConditionOpcode::AggSigMe, Box::new(|coin| coin.name().into_bytes()));

    let addendum = coin_to_addendum_f_lookup.get(&opcode).expect("Opcode not found")(coin);
    let additional_data = agg_sig_additional_data.get(&opcode).expect("Opcode not found").clone();

    [&msg[..], &addendum[..], &additional_data[..]].concat()
}
