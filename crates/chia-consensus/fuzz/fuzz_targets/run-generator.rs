#![no_main]
use chia_bls::Signature;
use chia_consensus::allocator::make_allocator;
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::run_block_generator::{run_block_generator, run_block_generator2};
use chia_consensus::validation_error::ValidationErr;
use clvmr::chia_dialect::LIMIT_HEAP;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut a1 = make_allocator(LIMIT_HEAP);
    let r1 = run_block_generator::<&[u8], _>(
        &mut a1,
        data,
        [],
        110_000_000,
        0,
        &Signature::default(),
        None,
        &TEST_CONSTANTS,
    );
    drop(a1);

    let mut a2 = make_allocator(LIMIT_HEAP);
    let r2 = run_block_generator2::<&[u8], _>(
        &mut a2,
        data,
        [],
        110_000_000,
        0,
        &Signature::default(),
        None,
        &TEST_CONSTANTS,
    );
    drop(a2);

    #[allow(clippy::match_same_arms)]
    match (r1, r2) {
        (Err(ValidationErr::CostExceeded), Ok(_)) => {
            // Since run_block_generator2 cost less, it's not a problem if the
            // original generator runs out of cost while the rust implementation
            // succeeds. This is part of its features.
        }
        (Err(_), Err(_)) => {
            // The specific error may not match, because
            // run_block_generator2() parses conditions after each spend
            // instead of after running all spends
        }
        (Ok(a), Ok(b)) => {
            assert!(a.cost >= b.cost);
            assert!(a.execution_cost > b.execution_cost);
            assert_eq!(a.condition_cost, b.condition_cost);
            assert_eq!(a.reserve_fee, b.reserve_fee);
            assert_eq!(a.removal_amount, b.removal_amount);
            assert_eq!(a.addition_amount, b.addition_amount);
        }
        (r1, r2) => {
            println!("mismatching result");
            println!(" run_block_generator: {:?}", &r1);
            println!("run_block_generator2: {:?}", &r2);
            panic!("failed");
        }
    }
});
