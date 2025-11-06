#![no_main]
use chia_consensus::conditions::{
    parse_conditions, MempoolVisitor, ParseState, SpendBundleConditions, SpendConditions,
    ELIGIBLE_FOR_FF,
};
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::fast_forward::fast_forward_singleton;
use chia_consensus::spend_visitor::SpendVisitor;
use chia_consensus::validation_error::{ErrorCode, ValidationErr};
use chia_protocol::Bytes32;
use chia_protocol::Coin;
use chia_protocol::CoinSpend;
use chia_traits::streamable::Streamable;
use clvm_traits::ToClvm;
use clvm_utils::tree_hash;
use clvmr::serde::{node_from_bytes, node_to_bytes};
use clvmr::{Allocator, NodePtr};
use hex_literal::hex;
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

use clvmr::chia_dialect::ChiaDialect;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;

fuzz_target!(|data: &[u8]| {
    let Ok(spend) = CoinSpend::parse::<false>(&mut Cursor::new(data)) else {
        return;
    };
    let new_parents_parent =
        hex!("abababababababababababababababababababababababababababababababab");

    let mut a = Allocator::new_limited(500_000_000);
    let Ok(puzzle) = spend.puzzle_reveal.to_clvm(&mut a) else {
        return;
    };
    let Ok(solution) = spend.solution.to_clvm(&mut a) else {
        return;
    };
    let puzzle_hash = Bytes32::from(tree_hash(&a, puzzle));

    for new_amount in [0, 2, 3] {
        for new_parent_amount in [0, 2, 3] {
            let new_parent_coin = Coin {
                parent_coin_info: new_parents_parent.into(),
                puzzle_hash,
                amount: if new_parent_amount == 0 {
                    spend.coin.amount
                } else {
                    new_parent_amount
                },
            };

            let new_coin = Coin {
                parent_coin_info: new_parent_coin.coin_id(),
                puzzle_hash,
                amount: if new_amount == 0 {
                    spend.coin.amount
                } else {
                    new_amount
                },
            };

            test_ff(
                &mut a,
                &spend,
                &new_coin,
                &new_parent_coin,
                solution,
                puzzle,
            );
        }
    }
});

fn run_puzzle(
    a: &mut Allocator,
    puzzle: &[u8],
    solution: &[u8],
    parent_id: &[u8],
    amount: u64,
) -> core::result::Result<SpendBundleConditions, ValidationErr> {
    let puzzle = node_from_bytes(a, puzzle)?;
    let solution = node_from_bytes(a, solution)?;

    let dialect = ChiaDialect::new(0);
    let max_cost = 11_000_000_000;
    let Reduction(clvm_cost, conditions) = run_program(a, &dialect, puzzle, solution, max_cost)?;

    let mut ret = SpendBundleConditions {
        removal_amount: amount as u128,
        ..Default::default()
    };
    let mut state = ParseState::default();

    let puzzle_hash = tree_hash(a, puzzle);
    let coin_id = Coin::new(parent_id.try_into().unwrap(), puzzle_hash.into(), amount).coin_id();

    let mut spend = SpendConditions::new(
        a.new_atom(parent_id)?,
        amount,
        a.new_atom(&puzzle_hash)?,
        coin_id,
        0,
    );

    let mut visitor = MempoolVisitor::new_spend(&mut spend);

    let mut cost_left = max_cost - clvm_cost;
    parse_conditions(
        a,
        &mut ret,
        &mut state,
        spend,
        conditions,
        0,
        &mut cost_left,
        &TEST_CONSTANTS,
        &mut visitor,
    )?;
    ret.cost = max_cost - cost_left;
    Ok(ret)
}
fn test_ff(
    a: &mut Allocator,
    spend: &CoinSpend,
    new_coin: &Coin,
    new_parent_coin: &Coin,
    solution: NodePtr,
    puzzle: NodePtr,
) {
    // perform fast-forward
    let Ok(new_solution) =
        fast_forward_singleton(a, puzzle, solution, &spend.coin, new_coin, new_parent_coin)
    else {
        return;
    };
    let new_solution = node_to_bytes(a, new_solution).expect("serialize new solution");

    // run original spend
    let conditions1 = run_puzzle(
        a,
        spend.puzzle_reveal.as_slice(),
        spend.solution.as_slice(),
        &spend.coin.parent_coin_info,
        spend.coin.amount,
    );

    // run new spend
    let conditions2 = run_puzzle(
        a,
        spend.puzzle_reveal.as_slice(),
        new_solution.as_slice(),
        &new_coin.parent_coin_info,
        new_coin.amount,
    );

    // These are the kinds of failures that can happen because of the
    // fast-forward. It's OK to fail in different ways before and after, as long
    // as it's one of these failures
    let discrepancy_errors = [
        ErrorCode::AssertMyParentIdFailed,
        ErrorCode::AssertMyCoinIdFailed,
    ];

    match (conditions1, conditions2) {
        (Err(ValidationErr(n1, msg1)), Err(ValidationErr(n2, msg2))) => {
            if msg1 != msg2 || node_to_bytes(a, n1).unwrap() != node_to_bytes(a, n2).unwrap() {
                assert!(discrepancy_errors.contains(&msg1) || discrepancy_errors.contains(&msg2));
            }
        }
        (Ok(conditions1), Ok(conditions2)) => {
            assert_eq!(conditions1.reserve_fee, conditions2.reserve_fee);
            assert_eq!(conditions1.height_absolute, conditions2.height_absolute);
            assert_eq!(conditions1.seconds_absolute, conditions2.seconds_absolute);
            assert_eq!(conditions1.agg_sig_unsafe, conditions2.agg_sig_unsafe);
            assert_eq!(
                conditions1.before_height_absolute,
                conditions2.before_height_absolute
            );
            assert_eq!(
                conditions1.before_seconds_absolute,
                conditions2.before_seconds_absolute
            );
            assert_eq!(conditions1.cost, conditions2.cost);

            let spend1 = &conditions1.spends[0];
            let spend2 = &conditions2.spends[0];
            assert_eq!(spend1.create_coin, spend2.create_coin);
            assert_eq!(spend1.coin_amount, spend.coin.amount);
            assert_eq!(spend2.coin_amount, new_coin.amount);
            assert!(a.atom_eq(spend1.puzzle_hash, spend2.puzzle_hash));

            assert_eq!(spend1.height_relative, spend2.height_relative);
            assert_eq!(spend1.seconds_relative, spend2.seconds_relative);
            assert_eq!(spend1.before_height_relative, spend2.before_height_relative);
            assert_eq!(
                spend1.before_seconds_relative,
                spend2.before_seconds_relative
            );
            assert_eq!(spend1.birth_height, spend2.birth_height);
            assert_eq!(spend1.birth_seconds, spend2.birth_seconds);
            assert_eq!(spend1.create_coin, spend2.create_coin);
            assert_eq!(spend1.flags, spend2.flags);
        }
        (Ok(conditions1), Err(ValidationErr(_n2, msg2))) => {
            // if the spend is valid and becomes invalid when
            // rebased/fast-forwarded, it should at least not be considered
            // eligible.
            assert!((conditions1.spends[0].flags & ELIGIBLE_FOR_FF) == 0);
            assert!(discrepancy_errors.contains(&msg2));
        }
        (Err(ValidationErr(_n1, msg1)), Ok(conditions2)) => {
            // if the spend is invalid and becomes valid when
            // rebased/fast-forwarded, it should not be considered
            // eligible. This is a bit of a far-fetched scenario, but could
            // happen if there's an ASSERT_MY_COINID that's only valid after the
            // fast-forward
            assert!((conditions2.spends[0].flags & ELIGIBLE_FOR_FF) == 0);
            assert!(discrepancy_errors.contains(&msg1));
        }
    }
}
