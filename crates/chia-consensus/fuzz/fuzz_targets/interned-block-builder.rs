#![no_main]
use chia_bls::Signature;
use chia_consensus::build_interned_block::InternedBlockBuilder;
use chia_consensus::conditions::SpendBundleConditions;
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::flags::{ConsensusFlags, MEMPOOL_MODE};
use chia_consensus::owned_conditions::{OwnedSpendBundleConditions, OwnedSpendConditions};
use chia_consensus::run_block_generator::{get_coinspends_for_trusted_block, run_block_generator2};
use chia_consensus::solution_generator::solution_generator_backrefs;
use chia_protocol::{Bytes, CoinSpend, Program, SpendBundle};
use clvmr::{
    Allocator,
    chia_dialect::ChiaDialect,
    reduction::Reduction,
    run_program::run_program,
    serde::{node_from_bytes_backrefs, node_to_bytes},
};
use libfuzzer_sys::{Corpus, fuzz_target};

/// CLVM execution cost for each spend's puzzle+solution, matching what
/// `run_block_generator2` charges (without requiring a valid coin puzzle hash).
fn puzzle_execution_cost(spends: &[CoinSpend]) -> Result<u64, ()> {
    let mut a = Allocator::new();
    let dialect = ChiaDialect::new(MEMPOOL_MODE.to_clvm_flags());
    let mut cost_left = TEST_CONSTANTS.max_block_cost_clvm;
    let mut total = 0u64;
    for spend in spends {
        let puzzle =
            node_from_bytes_backrefs(&mut a, spend.puzzle_reveal.as_ref()).map_err(|_| ())?;
        let solution = node_from_bytes_backrefs(&mut a, spend.solution.as_ref()).map_err(|_| ())?;
        let Reduction(cost, _) =
            run_program(&mut a, &dialect, puzzle, solution, cost_left).map_err(|_| ())?;
        total += cost;
        cost_left = cost_left.saturating_sub(cost);
    }
    Ok(total)
}

/// Same normalization as `additional_tests.rs::normalized_spends`: sort by
/// coin_id, sort each spend's create_coin, and zero out fields that
/// legitimately differ between the interned and classic encodings.
fn normalized_spends(
    allocator: &Allocator,
    conds: SpendBundleConditions,
) -> Vec<OwnedSpendConditions> {
    let mut conds = OwnedSpendBundleConditions::from(allocator, conds);
    conds.spends.sort_by_key(|s| s.coin_id);
    for s in &mut conds.spends {
        s.create_coin.sort();
        s.flags = 0;
        s.fingerprint = Bytes::default();
    }
    conds.spends
}

fuzz_target!(|spends: Vec<CoinSpend>| -> Corpus {
    if spends.is_empty() {
        return Corpus::Reject;
    }

    // Reject inputs that won't parse (same filter as solution-generator).
    let mut a = Allocator::new();
    for spend in &spends {
        if node_from_bytes_backrefs(&mut a, spend.puzzle_reveal.as_ref()).is_err()
            || node_from_bytes_backrefs(&mut a, spend.solution.as_ref()).is_err()
        {
            return Corpus::Reject;
        }
    }

    let Ok(exec_cost) = puzzle_execution_cost(&spends) else {
        return Corpus::Reject;
    };

    let spend_bundle = SpendBundle {
        coin_spends: spends.clone(),
        aggregated_signature: Signature::default(),
    };

    let mut builder = InternedBlockBuilder::new(&TEST_CONSTANTS);
    let Ok((added, _)) = builder.add_spend_bundles([spend_bundle], exec_cost) else {
        return Corpus::Reject;
    };
    if !added {
        return Corpus::Reject;
    }

    let upper_bound = builder.cost();
    let Ok((generator, signature, cost)) = builder.finalize() else {
        return Corpus::Reject;
    };

    assert!(
        upper_bound >= cost,
        "cost() upper bound {upper_bound} must be >= finalize() cost {cost}"
    );

    let Ok((interned_a, conds)) = run_block_generator2::<&[u8], _>(
        generator.as_slice(),
        [],
        TEST_CONSTANTS.max_block_cost_clvm,
        MEMPOOL_MODE | ConsensusFlags::INTERNED_GENERATOR,
        &signature,
        None,
        &TEST_CONSTANTS,
    ) else {
        return Corpus::Reject;
    };

    assert_eq!(
        conds.cost, cost,
        "finalize() cost must match consensus INTERNED_GENERATOR path"
    );

    // Independently build a classic-serialization reference generator for
    // the same spends (cf. additional_tests.rs::build_classic_reference) and
    // run it under classic (non-INTERNED_GENERATOR) rules. Cost legitimately
    // differs between the two encodings and isn't compared, but spends and
    // conditions must agree; either run erroring while the other succeeds is
    // a finding.
    let classic_spends: Vec<_> = spends
        .iter()
        .map(|s| (s.coin, s.puzzle_reveal.as_ref(), s.solution.as_ref()))
        .collect();
    let Ok(classic_generator) = solution_generator_backrefs(classic_spends) else {
        return Corpus::Reject;
    };

    let classic_result = run_block_generator2::<&[u8], _>(
        classic_generator.as_slice(),
        [],
        TEST_CONSTANTS.max_block_cost_clvm,
        MEMPOOL_MODE,
        &signature,
        None,
        &TEST_CONSTANTS,
    );

    match classic_result {
        Ok((classic_a, classic_conds)) => {
            assert_eq!(
                normalized_spends(&interned_a, conds),
                normalized_spends(&classic_a, classic_conds),
                "interned and classic generators disagree on spends for the same spend bundle"
            );
        }
        Err(e) => {
            panic!(
                "classic reference generator errored where the interned generator succeeded: {e:?}"
            );
        }
    }

    // Round-trip: generator bytes must decode back to the same spends (cf. generator.rs).
    // finalize() emits serde_2026, so INTERNED_GENERATOR is needed to parse it.
    let gen_prog = Program::new(generator.into());
    let Ok(mut result) = get_coinspends_for_trusted_block(
        &TEST_CONSTANTS,
        &gen_prog,
        vec![&[]],
        ConsensusFlags::INTERNED_GENERATOR,
    ) else {
        return Corpus::Reject;
    };

    assert_eq!(spends.len(), result.len());
    result.reverse();

    for (spend, res) in spends.iter().zip(result) {
        assert_eq!(res.coin.parent_coin_info, spend.coin.parent_coin_info);
        assert_eq!(res.coin.amount, spend.coin.amount);

        if res.puzzle_reveal != Program::default() {
            let node = node_from_bytes_backrefs(&mut a, spend.puzzle_reveal.as_ref())
                .expect("invalid puzzle reveal");
            let minimised_bytes = node_to_bytes(&a, node).expect("node_to_bytes");
            assert_eq!(res.puzzle_reveal, Program::new(minimised_bytes.into()));
        }
        if res.solution != Program::default() {
            let node = node_from_bytes_backrefs(&mut a, spend.solution.as_ref())
                .expect("invalid solution");
            let minimised_bytes = node_to_bytes(&a, node).expect("node_to_bytes");
            assert_eq!(res.solution, Program::new(minimised_bytes.into()));
        }
    }

    Corpus::Keep
});
