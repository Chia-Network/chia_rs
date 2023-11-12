#![no_main]
use chia::fast_forward::fast_forward_singleton;
use chia::gen::conditions::MempoolVisitor;
use chia::gen::run_puzzle::run_puzzle;
use chia::gen::validation_error::ValidationErr;
use chia_protocol::Bytes32;
use chia_protocol::Coin;
use chia_protocol::CoinSpend;
use chia_traits::streamable::Streamable;
use clvm_utils::tree_hash;
use clvmr::serde::node_from_bytes;
use clvmr::serde::node_to_bytes;
use clvmr::Allocator;
use hex_literal::hex;
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    let Ok(spend) = CoinSpend::parse(&mut Cursor::new(data)) else {
        return;
    };
    let new_parents_parent =
        hex!("abababababababababababababababababababababababababababababababab");

    let mut a = Allocator::new_limited(500000000, 62500000, 62500000);
    let Ok(puzzle) = node_from_bytes(&mut a, spend.puzzle_reveal.as_slice()) else {
        return;
    };
    let Ok(solution) = node_from_bytes(&mut a, spend.solution.as_slice()) else {
        return;
    };
    let puzzle_hash = Bytes32::from(tree_hash(&a, puzzle));

    let new_parent_coin = Coin {
        parent_coin_info: new_parents_parent.as_slice().into(),
        puzzle_hash,
        amount: spend.coin.amount,
    };

    let new_coin = Coin {
        parent_coin_info: new_parent_coin.coin_id().into(),
        puzzle_hash,
        amount: spend.coin.amount,
    };

    // perform fast-forward
    let Ok(new_solution) = fast_forward_singleton(
        &mut a,
        puzzle,
        solution,
        &spend.coin,
        &new_coin,
        &new_parent_coin,
    ) else {
        return;
    };
    let new_solution = node_to_bytes(&a, new_solution).expect("serialize new solution");

    // run original spend
    let conditions1 = run_puzzle::<MempoolVisitor>(
        &mut a,
        spend.puzzle_reveal.as_slice(),
        spend.solution.as_slice(),
        &spend.coin.parent_coin_info,
        spend.coin.amount,
        11000000000,
        0,
    );

    // run new spend
    let conditions2 = run_puzzle::<MempoolVisitor>(
        &mut a,
        spend.puzzle_reveal.as_slice(),
        new_solution.as_slice(),
        &new_coin.parent_coin_info,
        new_coin.amount,
        11000000000,
        0,
    );

    match (conditions1, conditions2) {
        (Err(ValidationErr(n1, msg1)), Err(ValidationErr(n2, msg2))) => {
            assert_eq!(msg1, msg2);
            assert_eq!(
                node_to_bytes(&a, n1).unwrap(),
                node_to_bytes(&a, n2).unwrap()
            );
        }
        (Ok(conditions1), Ok(conditions2)) => {
            assert!(conditions1.spends[0].create_coin == conditions2.spends[0].create_coin);
        }
        _ => {
            panic!("unexpected");
        }
    }
});
