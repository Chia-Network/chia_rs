use std::collections::HashMap;
use crate::gen::SpendBundleConditions;
use crate::gen::conditions::Condition;
use chia_bls::PublicKey;
use crate::gen::opcodes::{AGG_SIG_AMOUNT, AGG_SIG_PARENT, AGG_SIG_PARENT_PUZZLE, AGG_SIG_PUZZLE_AMOUNT,
    AGG_SIG_PUZZLE_AMOUNT, AGG_SIG_ME, AGG_SIG_UNSAFE, AGG_SIG_PUZZLE, AGG_SIG_PARENT_AMOUNT};

pub fn pkm_pairs(conditions: OwnedSpendBundleConditions, additional_data: &[u8]) -> Result<(Vec<PublicKey>, Vec<&[u8]>), Err> {
    let pks = Vec<PublicKey>::new();
    let msgs = Vec<&[u8]>::new();
    let disallowed = agg_sig_additional_data(additional_data);
    for (pk, msg) in conditions.agg_sig_unsafe {
        pks.push(pk);
        msgs.push(msg);
        for (_, disallowed_val) in disallowed.into_iter() {
            if msg.ends_with(disallowed_val) {
                return Err()
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
                msgs.push(make_aggsig_final_message(condition, &msg, spend, data));
            }
        }
    }
    Ok((pks, msgs))
}

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
        panic!("Expected Coin or Spend, got {:?}", spend); 
    }

    let mut coin_to_addendum_f_lookup: HashMap<ConditionOpcode, Box<dyn Fn(&Coin) -> Vec<u8>>> = HashMap::new();
    coin_to_addendum_f_lookup.insert(AGG_SIG_PARENT, Box::new(|coin| coin.parent_coin_info.clone()));
    coin_to_addendum_f_lookup.insert(AGG_SIG_PUZZLE, Box::new(|coin| coin.puzzle_hash.clone()));
    coin_to_addendum_f_lookup.insert(AGG_SIG_AMOUNT, Box::new(|coin| int_to_bytes(coin.amount)));
    coin_to_addendum_f_lookup.insert(AGG_SIG_PUZZLE_AMOUNT, Box::new(|coin| coin.puzzle_hash.clone() + &int_to_bytes(coin.amount)));
    coin_to_addendum_f_lookup.insert(AGG_SIG_PARENT_AMOUNT, Box::new(|coin| coin.parent_coin_info.clone() + &int_to_bytes(coin.amount)));
    coin_to_addendum_f_lookup.insert(AGG_SIG_PARENT_PUZZLE, Box::new(|coin| coin.parent_coin_info.clone() + &coin.puzzle_hash.clone()));
    coin_to_addendum_f_lookup.insert(AGG_SIG_ME, Box::new(|coin| coin.name().into_bytes()));

    let addendum = coin_to_addendum_f_lookup.get(&opcode).expect("Opcode not found")(coin);
    let additional_data = agg_sig_additional_data.get(&opcode).expect("Opcode not found").clone();

    [&msg[..], &addendum[..], &additional_data[..]].concat()
}

fn agg_sig_additional_data(agg_sig_data: &[u8]) -> HashMap<ConditionOpcode, Vec<u8>> {
    let mut ret: HashMap<ConditionOpcode, Vec<u8>> = HashMap::new();

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