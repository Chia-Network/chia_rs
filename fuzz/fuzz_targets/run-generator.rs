#![no_main]
use chia::allocator::make_allocator;
use chia::gen::flags::{ALLOW_BACKREFS, LIMIT_OBJECTS};
use chia::gen::run_block_generator::{run_block_generator, run_block_generator2};
use clvmr::chia_dialect::LIMIT_HEAP;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut a1 = make_allocator(LIMIT_HEAP | LIMIT_OBJECTS);
    let r1 = run_block_generator::<&[u8]>(&mut a1, data, &[], 110000000, ALLOW_BACKREFS);
    drop(a1);

    let mut a2 = make_allocator(LIMIT_HEAP | LIMIT_OBJECTS);
    let r2 = run_block_generator2::<&[u8]>(&mut a2, data, &[], 110000000, ALLOW_BACKREFS);
    drop(a2);

    match (r1, r2) {
        (Err(_), Err(_)) => {
            // The specific error may not match, because
            // run_block_generator2() parses conditions after each spend
            // instead of after running all spends
        }
        (Ok(a), Ok(b)) => {
            assert_eq!(a.cost, b.cost);
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
