use crate::conditions::{
    ELIGIBLE_FOR_DEDUP, MempoolVisitor, ParseState, SpendBundleConditions, process_single_spend,
    validate_conditions,
};
use crate::consensus_constants::ConsensusConstants;
use crate::flags::{ConsensusFlags, MEMPOOL_MODE};
use crate::puzzle_fingerprint::compute_puzzle_fingerprint;
use crate::run_block_generator::subtract_cost;
use crate::solution_generator::calculate_generator_length;
use crate::spend_visitor::SpendVisitor;
use crate::spendbundle_validation::get_flags_for_height_and_constants;
use crate::validation_error::ErrorCode;
use crate::validation_error::ValidationErr;
use chia_bls::PublicKey;
use chia_protocol::{Bytes, SpendBundle};

use clvm_utils::tree_hash;
use clvmr::allocator::Allocator;
use clvmr::chia_dialect::ChiaDialect;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;
use clvmr::serde::node_from_bytes;

const QUOTE_BYTES: usize = 2;

pub fn get_conditions_from_spendbundle(
    a: &mut Allocator,
    spend_bundle: &SpendBundle,
    max_cost: u64,
    prev_tx_height: u32,
    constants: &ConsensusConstants,
) -> Result<SpendBundleConditions, ValidationErr> {
    let flags = get_flags_for_height_and_constants(prev_tx_height, constants);
    Ok(run_spendbundle(
        a,
        spend_bundle,
        max_cost,
        flags | MEMPOOL_MODE | ConsensusFlags::DONT_VALIDATE_SIGNATURE,
        constants,
    )?
    .0)
}

// returns the conditions for the spendbundle, along with the (public key,
// message) pairs emitted by the spends (for validating the aggregate signature)
#[allow(clippy::type_complexity)]
pub fn run_spendbundle(
    a: &mut Allocator,
    spend_bundle: &SpendBundle,
    max_cost: u64,
    flags: ConsensusFlags,
    constants: &ConsensusConstants,
) -> Result<(SpendBundleConditions, Vec<(PublicKey, Bytes)>), ValidationErr> {
    // below is an adapted version of the code from run_block_generators::run_block_generator2()
    // it assumes no block references are passed in
    let mut cost_left = max_cost;
    let dialect = ChiaDialect::new(flags.to_clvm_flags());
    let mut ret = SpendBundleConditions::default();
    let mut state = ParseState::default();
    // We don't pay the size cost (nor execution cost) of being wrapped by a
    // quote (in solution_generator).
    let generator_length_without_quote =
        calculate_generator_length(&spend_bundle.coin_spends) - QUOTE_BYTES;

    let byte_cost = generator_length_without_quote as u64 * constants.cost_per_byte;
    subtract_cost(a, &mut cost_left, byte_cost)?;

    for coin_spend in &spend_bundle.coin_spends {
        // process the spend
        let puz = node_from_bytes(a, coin_spend.puzzle_reveal.as_slice())?;
        let sol = node_from_bytes(a, coin_spend.solution.as_slice())?;
        let parent = a.new_atom(coin_spend.coin.parent_coin_info.as_slice())?;
        let amount = a.new_number(coin_spend.coin.amount.into())?;
        let Reduction(clvm_cost, conditions) = run_program(a, &dialect, puz, sol, cost_left)?;

        ret.execution_cost += clvm_cost;
        subtract_cost(a, &mut cost_left, clvm_cost)?;

        let buf = tree_hash(a, puz);
        if coin_spend.coin.puzzle_hash != buf.into() {
            return Err(ValidationErr(puz, ErrorCode::WrongPuzzleHash));
        }
        let puzzle_hash = a.new_atom(&buf)?;
        let spend = process_single_spend::<MempoolVisitor>(
            a,
            &mut ret,
            &mut state,
            parent,
            puzzle_hash,
            amount,
            conditions,
            flags,
            &mut cost_left,
            clvm_cost,
            constants,
        )?;

        if (spend.flags & ELIGIBLE_FOR_DEDUP) != 0
            && flags.contains(ConsensusFlags::COMPUTE_FINGERPRINT)
        {
            spend.fingerprint = compute_puzzle_fingerprint(a, conditions)?;
        }
    }

    MempoolVisitor::post_process(a, &state, &mut ret)?;
    validate_conditions(a, &ret, &state, a.nil(), flags)?;

    assert!(max_cost >= cost_left);
    ret.cost = max_cost - cost_left;
    Ok((ret, state.pkm_pairs))
}

#[cfg(test)]
mod tests {
    use crate::consensus_constants::TEST_CONSTANTS;

    use super::*;
    use crate::allocator::make_allocator;
    use crate::conditions::{ELIGIBLE_FOR_DEDUP, ELIGIBLE_FOR_FF};
    use crate::run_block_generator::run_block_generator2;
    use crate::solution_generator::solution_generator;
    use chia_bls::Signature;
    use chia_protocol::CoinSpend;
    use chia_traits::Streamable;
    use rstest::rstest;
    use std::fs::read;

    const QUOTE_EXECUTION_COST: u64 = 20;
    const QUOTE_BYTES_COST: u64 = QUOTE_BYTES as u64 * TEST_CONSTANTS.cost_per_byte;

    fn assert_run_spendbundle_matches_parse_spends(spend_bundle: &SpendBundle) {
        use crate::conditions::parse_spends;

        let flags = MEMPOOL_MODE | ConsensusFlags::DONT_VALIDATE_SIGNATURE;

        let mut a1 = make_allocator(ConsensusFlags::LIMIT_HEAP);
        let (sb_conds, _) = run_spendbundle(
            &mut a1,
            spend_bundle,
            11_000_000_000,
            flags,
            &TEST_CONSTANTS,
        )
        .expect("run_spendbundle");

        let mut a2 = make_allocator(ConsensusFlags::LIMIT_HEAP);
        let dialect = ChiaDialect::new(flags.to_clvm_flags());

        let mut spend_list = a2.nil();
        for coin_spend in spend_bundle.coin_spends.iter().rev() {
            let puz = node_from_bytes(&mut a2, coin_spend.puzzle_reveal.as_slice()).unwrap();
            let sol = node_from_bytes(&mut a2, coin_spend.solution.as_slice()).unwrap();
            let Reduction(_, conditions) =
                run_program(&mut a2, &dialect, puz, sol, 11_000_000_000).unwrap();

            let parent = a2
                .new_atom(coin_spend.coin.parent_coin_info.as_slice())
                .unwrap();
            let ph_bytes = tree_hash(&a2, puz);
            let puzzle_hash = a2.new_atom(&ph_bytes).unwrap();
            let amount = a2.new_number(coin_spend.coin.amount.into()).unwrap();

            let nil = a2.nil();
            let tuple = a2.new_pair(conditions, nil).unwrap();
            let tuple = a2.new_pair(amount, tuple).unwrap();
            let tuple = a2.new_pair(puzzle_hash, tuple).unwrap();
            let tuple = a2.new_pair(parent, tuple).unwrap();

            spend_list = a2.new_pair(tuple, spend_list).unwrap();
        }

        let nil = a2.nil();
        let spends_node = a2.new_pair(spend_list, nil).unwrap();

        let ps_conds = parse_spends::<MempoolVisitor>(
            &a2,
            spends_node,
            11_000_000_000,
            0,
            flags,
            &Signature::default(),
            None,
            &TEST_CONSTANTS,
        )
        .expect("parse_spends");

        assert_eq!(
            sb_conds.spends.len(),
            ps_conds.spends.len(),
            "number of spends differ"
        );
        for (i, (s1, s2)) in sb_conds
            .spends
            .iter()
            .zip(ps_conds.spends.iter())
            .enumerate()
        {
            assert_eq!(
                s1.flags, s2.flags,
                "spend {i} flags differ: run_spendbundle={:#x}, parse_spends={:#x}",
                s1.flags, s2.flags
            );
        }
    }

    #[rstest]
    #[case("3000253", 8, 2, 51_216_870)]
    #[case("1000101", 34, 15, 250_083_677)]
    fn test_get_conditions_from_spendbundle(
        #[case] filename: &str,
        #[case] spends: usize,
        #[case] additions: usize,
        #[values(0, 1, 1_000_000, 5_000_000)] prev_tx_height: u32,
        #[case] cost: u64,
    ) {
        let bundle = SpendBundle::from_bytes(
            &read(format!("../../test-bundles/{filename}.bundle")).expect("read file"),
        )
        .expect("parse bundle");

        let mut a = make_allocator(ConsensusFlags::LIMIT_HEAP);
        let conditions =
            get_conditions_from_spendbundle(&mut a, &bundle, cost, prev_tx_height, &TEST_CONSTANTS)
                .expect("get_conditions_from_spendbundle");

        assert_eq!(conditions.spends.len(), spends);
        let create_coins = conditions
            .spends
            .iter()
            .fold(0, |sum, spend| sum + spend.create_coin.len());
        assert_eq!(create_coins, additions);
        assert_eq!(conditions.cost, cost);
        // Generate a block with the same spend bundle and compare its cost
        let program_spends = bundle.coin_spends.iter().map(|coin_spend| {
            (
                coin_spend.coin,
                &coin_spend.puzzle_reveal,
                &coin_spend.solution,
            )
        });
        let program = solution_generator(program_spends).expect("solution_generator failed");
        let blocks: &[&[u8]] = &[];
        let (_, block_conds) = run_block_generator2(
            program.as_slice(),
            blocks,
            11_000_000_000,
            MEMPOOL_MODE | ConsensusFlags::DONT_VALIDATE_SIGNATURE,
            &Signature::default(),
            None,
            &TEST_CONSTANTS,
        )
        .expect("run_block_generator2 failed");
        // The cost difference here is because get_conditions_from_spendbundle
        // does not include the overhead to make a block.
        assert_eq!(
            conditions.cost,
            block_conds.cost - QUOTE_EXECUTION_COST - QUOTE_BYTES_COST
        );

        assert_eq!(
            conditions.execution_cost,
            block_conds.execution_cost - QUOTE_EXECUTION_COST
        );
        assert_eq!(conditions.condition_cost, block_conds.condition_cost);

        assert_run_spendbundle_matches_parse_spends(&bundle);
    }

    #[rstest]
    #[case("bb13")]
    #[case("e3c0")]
    fn test_get_conditions_from_spendbundle_fast_forward(
        #[case] filename: &str,
        #[values(0, 1, 1_000_000, 5_000_000)] prev_tx_height: u32,
    ) {
        let cost = 77_341_866;
        let spend = CoinSpend::from_bytes(
            &read(format!("../../ff-tests/{filename}.spend")).expect("read file"),
        )
        .expect("parse Spend");

        let bundle = SpendBundle::new(vec![spend], Signature::default());

        let mut a = make_allocator(ConsensusFlags::LIMIT_HEAP);
        let conditions =
            get_conditions_from_spendbundle(&mut a, &bundle, cost, prev_tx_height, &TEST_CONSTANTS)
                .expect("get_conditions_from_spendbundle");

        assert_eq!(conditions.spends.len(), 1);
        let spend = &conditions.spends[0];
        assert_eq!(spend.flags, ELIGIBLE_FOR_FF | ELIGIBLE_FOR_DEDUP);
        assert_eq!(conditions.cost, cost);

        assert_run_spendbundle_matches_parse_spends(&bundle);
    }

    fn make_list(a: &mut Allocator, items: &[clvmr::NodePtr]) -> clvmr::NodePtr {
        let mut result = a.nil();
        for &item in items.iter().rev() {
            result = a.new_pair(item, result).unwrap();
        }
        result
    }

    fn condition_node(
        a: &mut Allocator,
        opcode: crate::opcodes::ConditionOpcode,
        args: &[clvmr::NodePtr],
    ) -> clvmr::NodePtr {
        let op = a.new_number(opcode.into()).unwrap();
        let mut items = vec![op];
        items.extend_from_slice(args);
        make_list(a, &items)
    }

    // The identity puzzle (CLVM atom 1): returns its solution as output.
    const IDENTITY_PUZZLE: &[u8] = &[1];

    fn identity_puzzle_hash() -> [u8; 32] {
        use clvm_utils::tree_hash_atom;
        tree_hash_atom(&[1]).to_bytes()
    }

    fn make_coin_spend(parent: [u8; 32], amount: u64, extra_conditions: &[&[u8]]) -> CoinSpend {
        use crate::opcodes::{ASSERT_MY_AMOUNT, CREATE_COIN};
        use chia_protocol::{Coin, Program};
        use clvmr::serde::{node_from_bytes, node_to_bytes};

        let mut a = Allocator::new();
        let puzzle_hash = identity_puzzle_hash();

        let ph_node = a.new_atom(&puzzle_hash).unwrap();
        let amt_node = a.new_number(amount.into()).unwrap();
        let create_coin = condition_node(&mut a, CREATE_COIN, &[ph_node, amt_node]);

        let amt_node2 = a.new_number(amount.into()).unwrap();
        let assert_my_amount = condition_node(&mut a, ASSERT_MY_AMOUNT, &[amt_node2]);

        let mut all = vec![create_coin, assert_my_amount];
        for extra in extra_conditions {
            all.push(node_from_bytes(&mut a, extra).unwrap());
        }
        let conditions = make_list(&mut a, &all);
        let solution = node_to_bytes(&a, conditions).unwrap();

        CoinSpend::new(
            Coin::new(parent.into(), puzzle_hash.into(), amount),
            Program::from(IDENTITY_PUZZLE.to_vec()),
            Program::from(solution),
        )
    }

    fn serialize_condition(
        opcode: crate::opcodes::ConditionOpcode,
        args: &[clvmr::NodePtr],
        a: &Allocator,
    ) -> Vec<u8> {
        use clvmr::serde::node_to_bytes;
        let mut a2 = Allocator::new();
        let op = a2.new_number(opcode.into()).unwrap();
        let mut items = vec![op];
        for &arg in args {
            let bytes = node_to_bytes(a, arg).unwrap();
            items.push(node_from_bytes(&mut a2, &bytes).unwrap());
        }
        let list = make_list(&mut a2, &items);
        node_to_bytes(&a2, list).unwrap()
    }

    #[test]
    fn test_post_process_single_ff_eligible_spend() {
        let spend_a = make_coin_spend([1u8; 32], 123, &[]);

        let bundle = SpendBundle::new(vec![spend_a], Signature::default());
        let mut alloc = make_allocator(ConsensusFlags::LIMIT_HEAP);
        let flags = MEMPOOL_MODE | ConsensusFlags::DONT_VALIDATE_SIGNATURE;
        let (conds, _) =
            run_spendbundle(&mut alloc, &bundle, 11_000_000_000, flags, &TEST_CONSTANTS)
                .expect("run_spendbundle");

        assert_eq!(conds.spends.len(), 1);
        assert_ne!(conds.spends[0].flags & ELIGIBLE_FOR_FF, 0);
        assert_ne!(conds.spends[0].flags & ELIGIBLE_FOR_DEDUP, 0);

        assert_run_spendbundle_matches_parse_spends(&bundle);
    }

    #[test]
    fn test_post_process_assert_concurrent_spend_clears_ff() {
        use chia_protocol::Coin;

        let puzzle_hash = identity_puzzle_hash();
        let spend_a = make_coin_spend([1u8; 32], 123, &[]);
        let coin_a_id = Coin::new([1u8; 32].into(), puzzle_hash.into(), 123).coin_id();

        let mut a = Allocator::new();
        let coin_id_node = a.new_atom(coin_a_id.as_slice()).unwrap();
        let assert_concurrent =
            serialize_condition(crate::opcodes::ASSERT_CONCURRENT_SPEND, &[coin_id_node], &a);
        let spend_b = make_coin_spend([2u8; 32], 123, &[&assert_concurrent]);

        let bundle = SpendBundle::new(vec![spend_a, spend_b], Signature::default());
        let mut alloc = make_allocator(ConsensusFlags::LIMIT_HEAP);
        let flags = MEMPOOL_MODE | ConsensusFlags::DONT_VALIDATE_SIGNATURE;
        let (conds, _) =
            run_spendbundle(&mut alloc, &bundle, 11_000_000_000, flags, &TEST_CONSTANTS)
                .expect("run_spendbundle");

        assert_eq!(conds.spends.len(), 2);
        assert_eq!(conds.spends[0].flags & ELIGIBLE_FOR_FF, 0);
        assert_ne!(conds.spends[0].flags & ELIGIBLE_FOR_DEDUP, 0);

        assert_run_spendbundle_matches_parse_spends(&bundle);
    }

    #[test]
    fn test_post_process_ephemeral_output_clears_ff() {
        use chia_protocol::{Coin, Program};

        let puzzle_hash = identity_puzzle_hash();
        let spend_a = make_coin_spend([1u8; 32], 123, &[]);

        let coin_a_id = Coin::new([1u8; 32].into(), puzzle_hash.into(), 123).coin_id();
        let ephemeral_coin = Coin::new(coin_a_id, puzzle_hash.into(), 123);

        let a = Allocator::new();
        let nil = a.nil();
        let empty_solution = clvmr::serde::node_to_bytes(&a, nil).unwrap();

        let spend_b = CoinSpend::new(
            ephemeral_coin,
            Program::from(IDENTITY_PUZZLE.to_vec()),
            Program::from(empty_solution),
        );

        let bundle = SpendBundle::new(vec![spend_a, spend_b], Signature::default());
        let mut alloc = make_allocator(ConsensusFlags::LIMIT_HEAP);
        let flags = MEMPOOL_MODE | ConsensusFlags::DONT_VALIDATE_SIGNATURE;
        let (conds, _) =
            run_spendbundle(&mut alloc, &bundle, 11_000_000_000, flags, &TEST_CONSTANTS)
                .expect("run_spendbundle");

        assert_eq!(conds.spends.len(), 2);
        assert_eq!(conds.spends[0].flags & ELIGIBLE_FOR_FF, 0);

        assert_run_spendbundle_matches_parse_spends(&bundle);
    }

    // given a block generator and block-refs, convert run the generator to
    // produce the SpendBundle for the block without runningi, or validating,
    // the puzzles.
    fn convert_block_to_bundle(generator: &[u8], block_refs: &[Vec<u8>]) -> SpendBundle {
        use crate::run_block_generator::setup_generator_args;
        use chia_protocol::Bytes32;
        use chia_protocol::Coin;
        use chia_protocol::Program;
        use clvm_traits::{FromClvm, destructure_tuple, match_tuple};
        use clvm_utils::tree_hash_from_bytes;
        use clvmr::NodePtr;
        use clvmr::op_utils::first;
        use clvmr::serde::node_from_bytes_backrefs;

        let mut a = make_allocator(MEMPOOL_MODE);

        let generator = node_from_bytes_backrefs(&mut a, generator).expect("node_from_bytes");
        let args = setup_generator_args(&mut a, block_refs, ConsensusFlags::empty())
            .expect("setup_generator_args");
        let dialect = ChiaDialect::new(MEMPOOL_MODE.to_clvm_flags());
        let Reduction(_, mut all_spends) =
            run_program(&mut a, &dialect, generator, args, 11_000_000_000).expect("run_program");

        all_spends = first(&a, all_spends).expect("first");

        let mut spends = Vec::<CoinSpend>::new();

        // at this point all_spends is a list of:
        // (parent-coin-id puzzle-reveal amount solution . extra)
        // where extra may be nil, or additional extension data
        while let Some((spend, rest)) = a.next(all_spends) {
            all_spends = rest;
            // process the spend
            let destructure_tuple!(parent_id, puzzle, amount, solution, _) =
                <match_tuple!(Bytes32, Program, u64, Program, NodePtr)>::from_clvm(&a, spend)
                    .expect("parsing CLVM");
            spends.push(CoinSpend::new(
                Coin::new(
                    parent_id,
                    tree_hash_from_bytes(puzzle.as_ref()).expect("hash").into(),
                    amount,
                ),
                puzzle,
                solution,
            ));
        }
        SpendBundle::new(spends, Signature::default())
    }

    #[ignore = "expensive test, only run in release mode (--include-ignored)"]
    #[rstest]
    // this test requires running after hard fork 2, where the COST_CONDITIONS
    // flag is set
    // #[case("aa-million-messages")]
    #[case("new-agg-sigs")]
    #[case("infinity-g1")]
    #[case("block-1ee588dc")]
    #[case("block-6fe59b24")]
    #[case("block-b45268ac")]
    #[case("block-c2a8df0d")]
    #[case("block-e5002df2")]
    #[case("block-4671894")]
    #[case("block-225758")]
    #[case("assert-puzzle-announce-fail")]
    #[case("block-834752")]
    #[case("block-834752-compressed")]
    #[case("block-834760")]
    #[case("block-834761")]
    #[case("block-834765")]
    #[case("block-834766")]
    #[case("block-834768")]
    #[case("create-coin-different-amounts")]
    #[case("create-coin-hint-duplicate-outputs")]
    #[case("create-coin-hint")]
    #[case("create-coin-hint2")]
    #[case("deep-recursion-plus")]
    #[case("double-spend")]
    #[case("duplicate-coin-announce")]
    #[case("duplicate-create-coin")]
    #[case("duplicate-height-absolute-div")]
    #[case("duplicate-height-absolute-substr-tail")]
    #[case("duplicate-height-absolute-substr")]
    #[case("duplicate-height-absolute")]
    #[case("duplicate-height-relative")]
    #[case("duplicate-outputs")]
    #[case("duplicate-reserve-fee")]
    #[case("duplicate-seconds-absolute")]
    #[case("duplicate-seconds-relative")]
    #[case("height-absolute-ladder")]
    //#[case("infinite-recursion1")]
    //#[case("infinite-recursion2")]
    //#[case("infinite-recursion3")]
    //#[case("infinite-recursion4")]
    #[case("invalid-conditions")]
    #[case("just-puzzle-announce")]
    #[case("many-create-coin")]
    #[case("many-large-ints-negative")]
    #[case("many-large-ints")]
    #[case("max-height")]
    #[case("multiple-reserve-fee")]
    #[case("negative-reserve-fee")]
    //#[case("recursion-pairs")]
    #[case("unknown-condition")]
    #[case("duplicate-messages")]
    fn run_generator(#[case] name: &str) {
        use crate::test_generators::{print_conditions, print_diff};
        use std::fs::read_to_string;

        let filename = format!("../../generator-tests/{name}.txt");
        println!("file: {filename}");
        let test_file = read_to_string(filename).expect("test file not found");
        let (generator, expected) = test_file.split_once('\n').expect("invalid test file");
        let generator_buffer = hex::decode(generator).expect("invalid hex encoded generator");

        let expected = match expected.split_once("STRICT:\n") {
            Some((_, m)) => m,
            None => expected,
        };

        let mut block_refs = Vec::<Vec<u8>>::new();

        let filename = format!("../../generator-tests/{name}.env");
        if let Ok(env_hex) = read_to_string(&filename) {
            println!("block-ref file: {filename}");
            block_refs.push(hex::decode(env_hex).expect("hex decode env-file"));
        }

        let bundle = convert_block_to_bundle(&generator_buffer, &block_refs);

        // run the whole block through run_block_generator2() to ensure the
        // output conditions match and update the cost. The cost
        // of just the spend bundle will be lower
        let (execution_cost, block_cost, block_output, block_conds) = {
            let block_conds = run_block_generator2(
                &generator_buffer,
                &block_refs,
                11_000_000_000,
                MEMPOOL_MODE | ConsensusFlags::DONT_VALIDATE_SIGNATURE,
                &Signature::default(),
                None,
                &TEST_CONSTANTS,
            );
            match &block_conds {
                Ok((a2, conditions)) => (
                    conditions.execution_cost,
                    conditions.cost,
                    print_conditions(a2, conditions, a2),
                    block_conds,
                ),
                Err(code) => {
                    println!("error: {code:?}");
                    (
                        0,
                        0,
                        format!("FAILED: {}\n", u32::from(code.1)),
                        block_conds,
                    )
                }
            }
        };

        let mut a1 = make_allocator(MEMPOOL_MODE);
        let conds = get_conditions_from_spendbundle(
            &mut a1,
            &bundle,
            11_000_000_000,
            5_000_000,
            &TEST_CONSTANTS,
        );

        let output = match conds {
            Ok(mut conditions) => {
                // the cost of running the spend bundle should never be higher
                // than the whole block but it's likely less.
                // but only if the byte cost is not taken into account. The
                // block will likely be smaller because the compression makes it
                // smaller.
                let block_byte_cost = generator_buffer.len() as u64 * TEST_CONSTANTS.cost_per_byte;
                let program_spends = bundle.coin_spends.iter().map(|coin_spend| {
                    (
                        coin_spend.coin,
                        &coin_spend.puzzle_reveal,
                        &coin_spend.solution,
                    )
                });
                let generator_length_without_quote = solution_generator(program_spends)
                    .expect("solution_generator failed")
                    .len()
                    - QUOTE_BYTES;
                let bundle_byte_cost =
                    generator_length_without_quote as u64 * TEST_CONSTANTS.cost_per_byte;
                println!(
                    " block_cost: {block_cost} bytes: {block_byte_cost} exe-cost: {execution_cost}"
                );
                println!("bundle_cost: {} bytes: {bundle_byte_cost}", conditions.cost);
                println!("execution_cost: {}", conditions.execution_cost);
                println!("condition_cost: {}", conditions.condition_cost);
                assert!(conditions.cost - bundle_byte_cost <= block_cost - block_byte_cost);
                assert!(conditions.cost > 0);
                assert!(conditions.execution_cost > 0);
                assert_eq!(
                    conditions.cost,
                    conditions.condition_cost + conditions.execution_cost + bundle_byte_cost
                );
                // update the cost we print here, just to be compatible with
                // the test cases we have. We've already ensured the cost is
                // lower
                conditions.cost = block_cost;
                conditions.execution_cost = execution_cost;
                let (a2, _) = block_conds.as_ref().unwrap();
                print_conditions(&a1, &conditions, a2)
            }
            Err(code) => {
                println!("error: {code:?}");
                format!("FAILED: {}\n", u32::from(code.1))
            }
        };

        if output != block_output {
            print_diff(&output, &block_output);
            panic!(
                "run_block_generator2 produced a different result than get_conditions_from_spendbundle()"
            );
        }

        if output != expected {
            print_diff(&output, expected);
            panic!("mismatching condition output");
        }
    }
}
