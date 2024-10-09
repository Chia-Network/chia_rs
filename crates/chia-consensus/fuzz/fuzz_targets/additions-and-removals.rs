#![no_main]
use chia_bls::Signature;
use chia_consensus::allocator::make_allocator;
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::gen::additions_and_removals::additions_and_removals;
use chia_consensus::gen::flags::{ALLOW_BACKREFS, DONT_VALIDATE_SIGNATURE};
use chia_consensus::gen::run_block_generator::run_block_generator2;
use chia_protocol::{Bytes, Coin};
use libfuzzer_sys::fuzz_target;
use std::collections::HashSet;

fuzz_target!(|data: &[u8]| {
    // additions_and_removals only work on trusted blocks, so if
    // run_block_generator2() fails, we can call additions_and_removals() on it.
    let results = additions_and_removals::<&[u8], _>(data, [], ALLOW_BACKREFS, &TEST_CONSTANTS);

    let mut a1 = make_allocator(0);
    let Ok(r1) = run_block_generator2::<&[u8], _>(
        &mut a1,
        data,
        [],
        110_000_000,
        ALLOW_BACKREFS | DONT_VALIDATE_SIGNATURE,
        &Signature::default(),
        None,
        &TEST_CONSTANTS,
    ) else {
        // just because the full block execution fails, doesn't mean
        // additons_and_removals() failed. It assumes a valid block and may
        // return Ok even for invalid blocks.
        return;
    };

    // if run_block_generator() passed however, additions_and_removals() also
    // must pass
    let (additions, removals) = results.expect("additions_and_removals()");

    let mut expect_additions = HashSet::<(Coin, Option<Bytes>)>::new();
    let mut expect_removals = HashSet::<Coin>::new();

    for spend in &r1.spends {
        let removal = Coin {
            parent_coin_info: a1
                .atom(spend.parent_id)
                .as_ref()
                .try_into()
                .expect("CREATE_COIN parent id"),
            puzzle_hash: a1
                .atom(spend.puzzle_hash)
                .as_ref()
                .try_into()
                .expect("CREATE_COIN puzzle hash"),
            amount: spend.coin_amount,
        };
        let coin_id = removal.coin_id();
        expect_removals.insert(removal);
        for add in &spend.create_coin {
            let addition = Coin {
                parent_coin_info: coin_id,
                puzzle_hash: add.puzzle_hash,
                amount: add.amount,
            };
            let hint = if a1.atom_len(add.hint) == 32 {
                Some(Into::<Bytes>::into(a1.atom(add.hint).as_ref()))
            } else {
                None
            };
            expect_additions.insert((addition, hint));
        }
    }

    assert_eq!(expect_additions.len(), additions.len());
    assert_eq!(expect_removals.len(), removals.len());

    for a in &additions {
        assert!(expect_additions.contains(a));
    }

    for r in &removals {
        assert!(expect_removals.contains(r));
    }
});
