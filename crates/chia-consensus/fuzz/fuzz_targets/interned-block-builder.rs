#![no_main]
use chia_bls::Signature;
use chia_consensus::build_interned_block::InternedBlockBuilder;
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::flags::{ConsensusFlags, MEMPOOL_MODE};
use chia_consensus::run_block_generator::{get_coinspends_for_trusted_block, run_block_generator2};
use chia_protocol::{CoinSpend, Program, SpendBundle};
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

    let Ok((_, conds)) = run_block_generator2::<&[u8], _>(
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

    // Round-trip: generator bytes must decode back to the same spends (cf. generator.rs).
    let gen_prog = Program::new(generator.into());
    let Ok(mut result) = get_coinspends_for_trusted_block(
        &TEST_CONSTANTS,
        &gen_prog,
        vec![&[]],
        ConsensusFlags::empty(),
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
