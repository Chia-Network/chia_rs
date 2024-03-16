#![no_main]
use chia_consensus::fast_forward::fast_forward_singleton;
use chia_consensus::gen::conditions::{MempoolVisitor, ELIGIBLE_FOR_FF};
use chia_consensus::gen::run_puzzle::run_puzzle;
use chia_consensus::gen::validation_error::{ErrorCode, ValidationErr};
use chia_protocol::Bytes32;
use chia_protocol::Coin;
use chia_protocol::CoinSpend;
use chia_traits::streamable::Streamable;
use clvm_traits::ToNodePtr;
use clvm_utils::tree_hash;
use clvmr::serde::node_to_bytes;
use clvmr::{Allocator, NodePtr};
use hex_literal::hex;
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let Ok(spend) = CoinSpend::parse::<false>(&mut Cursor::new(data)) else {
        return;
    };
    let new_parents_parent =
        hex!("abababababababababababababababababababababababababababababababab");

    let mut a = Allocator::new_limited(500000000);
    let Ok(puzzle) = spend.puzzle_reveal.to_node_ptr(&mut a) else {
        return;
    };
    let Ok(solution) = spend.solution.to_node_ptr(&mut a) else {
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
    let conditions1 = run_puzzle::<MempoolVisitor>(
        a,
        spend.puzzle_reveal.as_slice(),
        spend.solution.as_slice(),
        &spend.coin.parent_coin_info,
        spend.coin.amount,
        11000000000,
        0,
    );

    // run new spend
    let conditions2 = run_puzzle::<MempoolVisitor>(
        a,
        spend.puzzle_reveal.as_slice(),
        new_solution.as_slice(),
        &new_coin.parent_coin_info,
        new_coin.amount,
        11000000000,
        0,
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
