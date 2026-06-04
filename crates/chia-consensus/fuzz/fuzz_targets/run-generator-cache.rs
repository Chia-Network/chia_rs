#![no_main]
use chia_bls::Signature;
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::flags::ConsensusFlags;
use chia_consensus::owned_conditions::OwnedSpendBundleConditions;
use chia_consensus::run_block_generator::run_block_generator2;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let base_flags = ConsensusFlags::LIMIT_HEAP;

    let r_no_cache = run_block_generator2::<&[u8], _>(
        data,
        [],
        110_000_000,
        base_flags,
        &Signature::default(),
        None,
        &TEST_CONSTANTS,
    );

    let r_cache = run_block_generator2::<&[u8], _>(
        data,
        [],
        110_000_000,
        base_flags | ConsensusFlags::CONDITIONS_CACHE,
        &Signature::default(),
        None,
        &TEST_CONSTANTS,
    );

    match (r_no_cache, r_cache) {
        (Err(e1), Err(e2)) => {
            assert_eq!(
                e1.error_code(),
                e2.error_code(),
                "error codes differ: no_cache={e1:?} cache={e2:?}"
            );
        }
        (Ok((a1, conds1)), Ok((a2, conds2))) => {
            let mut owned1 = OwnedSpendBundleConditions::from(&a1, conds1);
            let mut owned2 = OwnedSpendBundleConditions::from(&a2, conds2);
            // HashSet iteration order is non-deterministic; normalize before comparing
            for spend in &mut owned1.spends {
                spend.create_coin.sort();
            }
            for spend in &mut owned2.spends {
                spend.create_coin.sort();
            }
            assert_eq!(
                owned1, owned2,
                "conditions differ between cached and uncached runs"
            );
        }
        (r1, r2) => {
            panic!("one run succeeded and the other failed:\n  no_cache: {r1:?}\n  cache: {r2:?}");
        }
    }
});
