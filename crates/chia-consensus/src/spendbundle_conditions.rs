use crate::allocator::make_allocator;
use crate::consensus_constants::ConsensusConstants;
use crate::gen::conditions::{
    process_single_spend, validate_conditions, MempoolVisitor, ParseState, SpendBundleConditions,
};
use crate::gen::flags::MEMPOOL_MODE;
use crate::gen::owned_conditions::OwnedSpendBundleConditions;
use crate::gen::run_block_generator::subtract_cost;
use crate::gen::validation_error::ValidationErr;
use crate::spendbundle_validation::get_flags_for_height_and_constants;
use chia_protocol::SpendBundle;
use clvm_utils::tree_hash;
use clvmr::allocator::Allocator;
use clvmr::chia_dialect::ChiaDialect;
use clvmr::chia_dialect::LIMIT_HEAP;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;
use clvmr::serde::node_from_bytes;

pub fn get_conditions_from_spendbundle(
    spend_bundle: &SpendBundle,
    max_cost: u64,
    height: u32,
    constants: &ConsensusConstants,
) -> Result<OwnedSpendBundleConditions, ValidationErr> {
    let flags = get_flags_for_height_and_constants(height, constants) | MEMPOOL_MODE;

    // below is an adapted version of the code from run_block_generators::run_block_generator2()
    // it assumes no block references are passed in
    let mut cost_left = max_cost;
    let dialect = ChiaDialect::new(flags);
    let mut a: Allocator = make_allocator(LIMIT_HEAP);
    let mut ret = SpendBundleConditions::default();
    let mut state = ParseState::default();

    for coin_spend in &spend_bundle.coin_spends {
        // process the spend
        let puz = node_from_bytes(&mut a, coin_spend.puzzle_reveal.as_slice())?;
        let sol = node_from_bytes(&mut a, coin_spend.solution.as_slice())?;
        let parent = a.new_atom(coin_spend.coin.parent_coin_info.as_slice())?;
        let amount = a.new_number(coin_spend.coin.amount.into())?;
        let Reduction(clvm_cost, conditions) = run_program(&mut a, &dialect, puz, sol, cost_left)?;

        subtract_cost(&a, &mut cost_left, clvm_cost)?;

        let buf = tree_hash(&a, puz);
        let puzzle_hash = a.new_atom(&buf)?;
        process_single_spend::<MempoolVisitor>(
            &a,
            &mut ret,
            &mut state,
            parent,
            puzzle_hash,
            amount,
            conditions,
            flags,
            &mut cost_left,
            constants,
        )?;
    }

    validate_conditions(&a, &ret, state, a.nil(), flags)?;
    assert!(max_cost >= cost_left);
    ret.cost = max_cost - cost_left;
    let osbc = OwnedSpendBundleConditions::from(&a, ret);
    Ok(osbc)
}

#[cfg(test)]
mod tests {
    use crate::consensus_constants::TEST_CONSTANTS;

    use super::*;
    use chia_bls::Signature;
    use chia_protocol::{Coin, CoinSpend, Program};
    use hex_literal::hex;

    #[test]
    fn test_get_conditions_from_spendbundle() {
        let test_coin = Coin::new(
            hex!("4444444444444444444444444444444444444444444444444444444444444444").into(),
            hex!("3333333333333333333333333333333333333333333333333333333333333333").into(),
            1,
        );

        let solution = hex!("ffff31ffb0997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2ff8568656c6c6f8080").to_vec();
        // ((49 0x997cc43ed8788f841fcf3071f6f212b89ba494b6ebaf1bda88c3f9de9d968a61f3b7284a5ee13889399ca71a026549a2 "hello"))

        let spend = CoinSpend::new(test_coin, Program::new(vec![1_u8].into()), solution.into());

        let coin_spends: Vec<CoinSpend> = vec![spend];
        let spend_bundle = SpendBundle {
            coin_spends,
            aggregated_signature: Signature::default(),
        };
        let osbc = get_conditions_from_spendbundle(
            &spend_bundle,
            TEST_CONSTANTS.max_block_cost_clvm,
            236,
            &TEST_CONSTANTS,
        )
        .expect("test should pass");

        assert!(osbc.spends.len() == 1);
        assert!(osbc.agg_sig_unsafe.len() == 1);
    }
}
