use crate::run_block_generator::setup_generator_args;
use crate::run_block_generator::subtract_cost;
use chia_protocol::Coin;

use crate::allocator::make_allocator;
use crate::consensus_constants::ConsensusConstants;
use crate::validation_error::{atom, first, next, rest, ErrorCode, ValidationErr};
use chia_protocol::{Bytes, Bytes32};
use clvm_traits::FromClvm;
use clvm_utils::{tree_hash_cached, TreeCache};
use clvmr::allocator::NodePtr;
use clvmr::chia_dialect::ChiaDialect;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;
use clvmr::serde::node_from_bytes_backrefs;

/// Run a *trusted* block generator and return its additions and removals. This
/// function does not validate the block, it is assumed to be valid.
/// The returned vectors are additions (with hints) and removals (with
/// pre-computed coin IDs).
#[allow(clippy::type_complexity)]
pub fn additions_and_removals<GenBuf: AsRef<[u8]>, I: IntoIterator<Item = GenBuf>>(
    program: &[u8],
    block_refs: I,
    flags: u32,
    constants: &ConsensusConstants,
) -> Result<(Vec<(Coin, Option<Bytes>)>, Vec<(Bytes32, Coin)>), ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    let mut a = make_allocator(flags);
    let mut additions = Vec::<(Coin, Option<Bytes>)>::new();
    let mut removals = Vec::<(Bytes32, Coin)>::new();

    let mut cost_left = constants.max_block_cost_clvm;

    let program = node_from_bytes_backrefs(&mut a, program)?;

    let args = setup_generator_args(&mut a, block_refs)?;
    let dialect = ChiaDialect::new(flags);

    let Reduction(clvm_cost, all_spends) = run_program(&mut a, &dialect, program, args, cost_left)?;

    subtract_cost(&a, &mut cost_left, clvm_cost)?;
    let all_spends = first(&a, all_spends)?;

    let mut cache = TreeCache::default();
    // at this point all_spends is a list of:
    // (parent-coin-id puzzle-reveal amount solution . extra)
    // where extra may be nil, or additional extension data

    // first iterate over all puzzle reveals to find duplicate nodes, to know
    // what to memoize during tree hash computations. This is managed by
    // TreeCache
    let mut iter = all_spends;
    while let Some((spend, rest)) = a.next(iter) {
        iter = rest;
        let (_parent_id, (puzzle, _rest)) =
            <(NodePtr, (NodePtr, NodePtr))>::from_clvm(&a, spend)
                .map_err(|_| ValidationErr(spend, ErrorCode::InvalidCondition))?;
        cache.visit_tree(&a, puzzle);
    }

    let mut iter = all_spends;
    while let Some((spend, tail)) = a.next(iter) {
        iter = tail;
        // process the spend
        let (parent_id, (puzzle, (amount, (solution, _spend_level_extra)))) =
            <(Bytes32, (NodePtr, (u64, (NodePtr, NodePtr))))>::from_clvm(&a, spend)
                .map_err(|_| ValidationErr(spend, ErrorCode::InvalidCondition))?;

        let Reduction(clvm_cost, mut iter) =
            run_program(&mut a, &dialect, puzzle, solution, cost_left)?;

        subtract_cost(&a, &mut cost_left, clvm_cost)?;

        let puzzle_hash = tree_hash_cached(&a, puzzle, &mut cache);

        let coin = Coin {
            parent_coin_info: parent_id,
            puzzle_hash: puzzle_hash.into(),
            amount,
        };

        let spend_id = coin.coin_id();
        removals.push((spend_id, coin));

        while let Some((mut c, next)) = next(&a, iter)? {
            iter = next;
            let op = first(&a, c)?;
            let Ok(op) = atom(&a, op, ErrorCode::InvalidConditionOpcode) else {
                // unknown opcodes (including pairs) are simply ingnored in
                // consensus mode
                continue;
            };
            // CREATE_COIN
            if op.as_ref() != [51_u8] {
                continue;
            }
            c = rest(&a, c)?;

            let (puzzle_hash, (amount, hint)) = <(Bytes32, (u64, NodePtr))>::from_clvm(&a, c)
                .map_err(|_| ValidationErr(c, ErrorCode::InvalidCondition))?;

            let coin = Coin {
                parent_coin_info: spend_id,
                puzzle_hash,
                amount,
            };

            // there was another item in the list
            // the item was a cons-box, and params is the left-hand
            // side, the list element

            let hint =
                if let Ok(((hint, _), _)) = <((Bytes, NodePtr), NodePtr)>::from_clvm(&a, hint) {
                    if hint.len() <= 32 {
                        Some(hint)
                    } else {
                        None
                    }
                } else {
                    None
                };
            additions.push((coin, hint));
        }
    }

    Ok((additions, removals))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::consensus_constants::TEST_CONSTANTS;
    use crate::flags::DONT_VALIDATE_SIGNATURE;
    use crate::run_block_generator::run_block_generator2;
    use chia_bls::Signature;
    use rstest::rstest;
    use std::collections::HashSet;

    #[rstest]
    #[case("new-agg-sigs")]
    #[case("block-1ee588dc")]
    #[case("block-6fe59b24")]
    #[case("block-b45268ac")]
    #[case("block-c2a8df0d")]
    #[case("block-e5002df2")]
    #[case("block-4671894")]
    #[case("block-225758")]
    #[case("block-834752")]
    #[case("block-834752-compressed")]
    #[case("block-834760")]
    #[case("block-834761")]
    #[case("block-834765")]
    #[case("block-834766")]
    #[case("block-834768")]
    #[case("create-coin-different-amounts")]
    #[case("create-coin-hint")]
    #[case("create-coin-hint2")]
    #[case("duplicate-height-absolute-div")]
    #[case("just-puzzle-announce")]
    #[case("many-create-coin")]
    #[case("many-large-ints-negative")]
    #[case("max-height")]
    #[case("multiple-reserve-fee")]
    #[case("unknown-condition")]
    fn test_additions_and_removals(#[case] name: &str) {
        use std::fs::read_to_string;

        let filename = format!("../../generator-tests/{name}.txt");
        println!("file: {filename}");
        let test_file = read_to_string(filename).expect("test file not found");
        let (generator, _expected) = test_file.split_once('\n').expect("invalid test file");
        let generator = hex::decode(generator).expect("invalid hex encoded generator");

        let mut block_refs = Vec::<Vec<u8>>::new();

        let filename = format!("../../generator-tests/{name}.env");
        if let Ok(env_hex) = read_to_string(&filename) {
            println!("block-ref file: {filename}");
            block_refs.push(hex::decode(env_hex).expect("hex decode env-file"));
        }

        // we run the block using run_block_generator2() to extract the additions
        // and removals we *expect* to see
        // additions_and_removals only work on trusted blocks, so if
        // run_block_generator2() fails, we can call additions_and_removals() on it.
        let mut a = make_allocator(0);
        let conds = run_block_generator2(
            &mut a,
            &generator,
            &block_refs,
            11_000_000_000,
            DONT_VALIDATE_SIGNATURE,
            &Signature::default(),
            None,
            &TEST_CONSTANTS,
        )
        .expect("run_block_generator2()");

        let mut expect_additions = HashSet::<(Coin, Option<Bytes>)>::new();
        let mut expect_removals = HashSet::<Coin>::new();

        for spend in &conds.spends {
            let removal = Coin {
                parent_coin_info: a.atom(spend.parent_id).as_ref().try_into().unwrap(),
                puzzle_hash: a.atom(spend.puzzle_hash).as_ref().try_into().unwrap(),
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
                let hint = if add.hint != NodePtr::NIL && a.atom_len(add.hint) <= 32 {
                    Some(Into::<Bytes>::into(a.atom(add.hint).as_ref()))
                } else {
                    None
                };
                println!("expect : {addition:?} hint: {hint:?}");
                expect_additions.insert((addition, hint));
            }
        }

        // now run the function under test
        let (additions, removals) =
            additions_and_removals(&generator, &block_refs, 0, &TEST_CONSTANTS)
                .expect("additions_and_removals()");

        assert_eq!(expect_additions.len(), additions.len());
        assert_eq!(expect_removals.len(), removals.len());

        for a in &additions {
            println!("addition: {a:?}");
            assert!(expect_additions.contains(a));
        }

        for r in &removals {
            assert!(expect_removals.contains(&r.1));
        }
    }
}
