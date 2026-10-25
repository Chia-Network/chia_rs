use crate::allocator::make_allocator;
use crate::condition_sanitizers::parse_amount;
use crate::consensus_constants::ConsensusConstants;
use crate::flags::ConsensusFlags;
use crate::opcodes::{
    AGG_SIG_AMOUNT, AGG_SIG_ME, AGG_SIG_PARENT, AGG_SIG_PARENT_AMOUNT, AGG_SIG_PARENT_PUZZLE,
    AGG_SIG_PUZZLE, AGG_SIG_PUZZLE_AMOUNT, AGG_SIG_UNSAFE, CREATE_COIN,
};
use crate::run_block_generator::{
    check_generator_node, check_generator_quote, extract_n, setup_generator_args, subtract_cost,
};
use crate::serde_2026::node_from_bytes_2026_trusted;
use crate::validation_error::{ErrorCode, ValidationErr, atom, first, next, rest};
use chia_protocol::{Bytes, Bytes32, BytesImpl, Coin, CoinSpend, Program};
use clvm_traits::FromClvm;
use clvm_utils::{TreeCache, tree_hash_cached};
use clvmr::allocator::{Allocator, NodePtr, SExp};
use clvmr::chia_dialect::ChiaDialect;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;
use clvmr::serde::node_from_bytes_backrefs;

// Returns the spend list wrapper of a trusted block generator: the spend
// list in an outer list whose rest is reserved for extension data. With
// INTERNED_SPEND_LIST, the generator is a serde_2026 blob holding the spend
// list wrapper directly; generator references are disabled and rejected.
// Otherwise the classic quoted generator is checked and executed. The CLVM
// cost is discarded; only consensus paths account for it.
pub(crate) fn spend_list_wrapper_for_trusted_block<
    GenBuf: AsRef<[u8]>,
    I: IntoIterator<Item = GenBuf>,
>(
    a: &mut Allocator,
    generator: &[u8],
    refs: I,
    flags: ConsensusFlags,
    max_cost: u64,
) -> Result<NodePtr, ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    if flags.contains(ConsensusFlags::INTERNED_SPEND_LIST) {
        // the generator field is the serialized spend list wrapper, in the
        // shape run_program() would produce for the classic path.
        // INTERNED_SPEND_LIST disables generator references.
        if refs.into_iter().next().is_some() {
            return Err(ValidationErr::Err(ErrorCode::TooManyGeneratorRefs));
        }
        Ok(node_from_bytes_2026_trusted(a, generator)?)
    } else {
        check_generator_quote(generator, flags)?;
        let program = node_from_bytes_backrefs(a, generator)?;
        check_generator_node(a, program, flags)?;
        let args = setup_generator_args(a, refs, flags)?;
        let dialect = ChiaDialect::new(flags.to_clvm_flags());
        let Reduction(_clvm_cost, spend_list_wrapper) =
            run_program(a, &dialect, program, args, max_cost)
                .map_err(|_| ValidationErr::Err(ErrorCode::GeneratorRuntimeError))?;
        Ok(spend_list_wrapper)
    }
}

// this function is less capable of handling problematic generators as they are
// returning serialized puzzles, which may not be possible. They will simply ignore many of the bad cases.
pub fn get_coinspends_for_trusted_block<GenBuf: AsRef<[u8]>, I: IntoIterator<Item = GenBuf>>(
    constants: &ConsensusConstants,
    generator: &[u8],
    refs: I,
    flags: ConsensusFlags,
) -> Result<Vec<CoinSpend>, ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    let mut a = make_allocator(flags);
    let mut output = Vec::<CoinSpend>::new();
    let spend_list_wrapper = spend_list_wrapper_for_trusted_block(
        &mut a,
        generator,
        refs,
        flags,
        constants.max_block_cost_clvm,
    )?;
    let all_spends = first(&a, spend_list_wrapper)?;
    let mut cache = TreeCache::default();
    let mut iter = all_spends;
    while let Some((spend, rest)) = a.next(iter) {
        iter = rest;
        let Ok([_, puzzle, _]) = extract_n::<3>(&a, spend, ErrorCode::InvalidCondition) else {
            continue;
        };
        cache.visit_tree(&a, puzzle);
    }
    iter = all_spends;
    while let Some((spend, rest)) = a.next(iter) {
        iter = rest;
        let Ok([parent_id, puzzle, amount, solution, _spend_level_extra]) =
            extract_n::<5>(&a, spend, ErrorCode::InvalidCondition)
        else {
            continue; // if we fail at this step then maybe the generator was malicious - try other spends
        };
        let puzhash = tree_hash_cached(&a, puzzle, &mut cache);
        let parent_id = BytesImpl::<32>::from_clvm(&a, parent_id)
            .map_err(|_| ValidationErr::Err(ErrorCode::InvalidParentId))?;
        let coin = Coin::new(
            parent_id,
            puzhash.into(),
            parse_amount(&a, amount, ErrorCode::InvalidCoinAmount)?,
        );
        // This may fail for malicious generators, where the puzzle reveal or
        // solution reuses CLVM subtrees such that a plain serialization becomes
        // very large. from_clvm() fails if the resulting buffer is greater than
        // 2 MB
        let puzzle_program = Program::from_clvm(&a, puzzle).unwrap_or_default();
        let solution_program = Program::from_clvm(&a, solution).unwrap_or_default();
        let coinspend = CoinSpend::new(coin, puzzle_program, solution_program);
        output.push(coinspend);
    }
    Ok(output)
}

/// Maximum number of conditions per spend before we start dropping conditions
/// to keep JSON and other serialized output bounded. Only AGG_SIG_* and
/// CREATE_COIN conditions are added after this limit is reached.
const MAX_CONDITIONS_PER_SPEND: usize = 1024;

/// Returns true for condition opcodes that are safe to include even after
/// exceeding the soft limit. These conditions have cost associated with them, so
/// are already restricted.
fn is_high_priority_condition(op: u32) -> bool {
    u16::try_from(op).is_ok()
        && matches!(
            op as u16,
            AGG_SIG_PARENT
                | AGG_SIG_PUZZLE
                | AGG_SIG_AMOUNT
                | AGG_SIG_PUZZLE_AMOUNT
                | AGG_SIG_PARENT_AMOUNT
                | AGG_SIG_PARENT_PUZZLE
                | AGG_SIG_UNSAFE
                | AGG_SIG_ME
                | CREATE_COIN
        )
}

// this function returns a list of tuples (coinspend, conditions)
// conditions are formatted as a vec of tuples of (condition_opcode, args)
// this function is less capable of handling problematic generators as they are
// returning serialized puzzles, which may not be possible. They will simply
// ignore many of the bad cases.
#[allow(clippy::type_complexity)]
pub fn get_coinspends_with_conditions_for_trusted_block<
    GenBuf: AsRef<[u8]>,
    I: IntoIterator<Item = GenBuf>,
>(
    constants: &ConsensusConstants,
    generator: &[u8],
    refs: I,
    flags: ConsensusFlags,
) -> Result<Vec<(CoinSpend, Vec<(u32, Vec<Vec<u8>>)>)>, ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    let mut a = make_allocator(flags);
    let mut output = Vec::<(CoinSpend, Vec<(u32, Vec<Vec<u8>>)>)>::new();
    let dialect = ChiaDialect::new(flags.to_clvm_flags());
    let spend_list_wrapper = spend_list_wrapper_for_trusted_block(
        &mut a,
        generator,
        refs,
        flags,
        constants.max_block_cost_clvm,
    )?;
    let all_spends = first(&a, spend_list_wrapper)?;
    let mut cache = TreeCache::default();
    let mut iter = all_spends;
    while let Some((spend, rest)) = a.next(iter) {
        iter = rest;
        let [_, puzzle, _] = extract_n::<3>(&a, spend, ErrorCode::InvalidCondition)?;
        cache.visit_tree(&a, puzzle);
    }
    iter = all_spends;
    while let Some((spend, rest)) = a.next(iter) {
        iter = rest;
        let mut cond_output = Vec::<(u32, Vec<Vec<u8>>)>::new();
        let Ok([parent_id, puzzle, amount, solution, _spend_level_extra]) =
            extract_n::<5>(&a, spend, ErrorCode::InvalidCondition)
        else {
            continue; // if we fail at this step then maybe the generator was malicious - try other spends
        };
        let puzhash = tree_hash_cached(&a, puzzle, &mut cache);
        let parent_id = BytesImpl::<32>::from_clvm(&a, parent_id)
            .map_err(|_| ValidationErr::Err(ErrorCode::InvalidParentId))?;
        let coin = Coin::new(
            parent_id,
            puzhash.into(),
            parse_amount(&a, amount, ErrorCode::InvalidCoinAmount)?,
        );
        let puzzle_program = Program::from_clvm(&a, puzzle).unwrap_or_default();
        let solution_program = Program::from_clvm(&a, solution).unwrap_or_default();

        let Reduction(_clvm_cost, res) = run_program(
            &mut a,
            &dialect,
            puzzle,
            solution,
            constants.max_block_cost_clvm,
        )
        .map_err(|_| ValidationErr::Err(ErrorCode::GeneratorRuntimeError))?;
        // conditions_list is the full returned output of puzzle ran with solution
        // ((51 0xcafef00d 100) (51 0x1234 200) ...)

        // condition is each grouped list
        // (51 0xcafef00d 100)
        let mut iter_two = res;
        'outer: while let Some((condition, rest_two)) = a.next(iter_two) {
            iter_two = rest_two;
            let mut iter_three = condition;
            let Some((condition_values, rest_three)) = a.next(iter_three) else {
                continue;
            };
            iter_three = rest_three;
            let Some(opcode) = a.small_number(condition_values) else {
                continue;
            };
            let mut bytes_vec = Vec::<Vec<u8>>::new();
            'inner: while let Some((condition_values, rest_three)) = a.next(iter_three) {
                iter_three = rest_three;
                if bytes_vec.len() < 6 {
                    if let SExp::Atom = a.sexp(condition_values) {
                        // a reasonable max length of an atom is 1,024 bytes
                        if a.atom_len(condition_values) >= 1024 {
                            // skip this condition
                            continue 'outer;
                        }
                        let bytes = a.atom(condition_values).to_vec();
                        bytes_vec.push(bytes);
                    }
                } else {
                    break 'inner; // we only care about the first 5 condition arguments
                }
            }

            // When over the per-spend limit, drop low-priority conditions first (REMARK,
            // announcements, SOFTFORK, SEND_MESSAGE, RECEIVE_MESSAGE) to keep output bounded.
            if cond_output.len() >= MAX_CONDITIONS_PER_SPEND && !is_high_priority_condition(opcode)
            {
                continue 'outer;
            }
            cond_output.push((opcode, bytes_vec));
        }
        output.push((
            CoinSpend::new(coin, puzzle_program, solution_program),
            cond_output,
        ));
    }
    Ok(output)
}

/// Run a *trusted* block generator and return its additions and removals. This
/// function does not validate the block, it is assumed to be valid.
/// The returned vectors are additions (with hints) and removals (with
/// pre-computed coin IDs).
#[allow(clippy::type_complexity)]
pub fn additions_and_removals<GenBuf: AsRef<[u8]>, I: IntoIterator<Item = GenBuf>>(
    program: &[u8],
    block_refs: I,
    flags: ConsensusFlags,
    constants: &ConsensusConstants,
) -> Result<(Vec<(Coin, Option<Bytes>)>, Vec<(Bytes32, Coin)>), ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    let mut a = make_allocator(flags);
    let mut additions = Vec::<(Coin, Option<Bytes>)>::new();
    let mut removals = Vec::<(Bytes32, Coin)>::new();

    let mut cost_left = constants.max_block_cost_clvm;

    let dialect = ChiaDialect::new(flags.to_clvm_flags());

    let spend_list_wrapper =
        spend_list_wrapper_for_trusted_block(&mut a, program, block_refs, flags, cost_left)?;
    let all_spends = first(&a, spend_list_wrapper)?;
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
                .map_err(|_| ValidationErr::Err(ErrorCode::InvalidCondition))?;
        cache.visit_tree(&a, puzzle);
    }

    let mut iter = all_spends;
    while let Some((spend, tail)) = a.next(iter) {
        iter = tail;
        // process the spend
        let (parent_id, (puzzle, (amount, (solution, _spend_level_extra)))) =
            <(Bytes32, (NodePtr, (NodePtr, (NodePtr, NodePtr))))>::from_clvm(&a, spend)
                .map_err(|_| ValidationErr::Err(ErrorCode::InvalidCondition))?;
        let amount = parse_amount(&a, amount, ErrorCode::InvalidCoinAmount)?;

        let Reduction(clvm_cost, mut iter) =
            run_program(&mut a, &dialect, puzzle, solution, cost_left)?;

        subtract_cost(&mut cost_left, clvm_cost)?;

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
            let Ok(op) = atom(
                &a,
                op,
                ValidationErr::Err(ErrorCode::InvalidConditionOpcode),
            ) else {
                // unknown opcodes (including pairs) are simply ingnored in
                // consensus mode
                continue;
            };
            // CREATE_COIN
            if op.as_ref() != [51_u8] {
                continue;
            }
            c = rest(&a, c)?;

            let (puzzle_hash, (amount, hint)) =
                <(Bytes32, (NodePtr, NodePtr))>::from_clvm(&a, c)
                    .map_err(|_| ValidationErr::Err(ErrorCode::InvalidCondition))?;
            let amount = parse_amount(&a, amount, ErrorCode::InvalidCoinAmount)?;

            let coin = Coin {
                parent_coin_info: spend_id,
                puzzle_hash,
                amount,
            };

            // there was another item in the list
            // the item was a cons-box, and params is the left-hand
            // side, the list element

            let hint =
                if let Ok(((hint, _), _)) = <((NodePtr, NodePtr), NodePtr)>::from_clvm(&a, hint) {
                    if let SExp::Atom = a.sexp(hint) {
                        if a.atom_len(hint) <= 32 {
                            Some(Into::<Bytes>::into(a.atom(hint).as_ref()))
                        } else {
                            None
                        }
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
mod tests {
    use super::*;
    use crate::consensus_constants::TEST_CONSTANTS;
    use crate::run_block_generator::run_block_generator2;
    use crate::solution_generator::solution_generator;
    use chia_bls::Signature;
    use clvm_utils::tree_hash_atom;
    use rstest::rstest;
    use std::collections::HashSet;

    const IDENTITY_PUZZLE: &[u8] = &[1];

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
        let (a, conds) = run_block_generator2(
            &generator,
            &block_refs,
            11_000_000_000,
            ConsensusFlags::DONT_VALIDATE_SIGNATURE,
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
        let (additions, removals) = additions_and_removals(
            &generator,
            &block_refs,
            ConsensusFlags::empty(),
            &TEST_CONSTANTS,
        )
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

    fn make_create_coin_generator(hint_size: usize) -> Vec<u8> {
        use crate::solution_generator::solution_generator;
        use clvm_traits::ToClvm;
        use clvmr::allocator::Allocator;
        use clvmr::serde::node_to_bytes;

        let mut a = Allocator::new();

        let hint = Bytes::new(vec![0x42u8; hint_size]);
        let ph = Bytes32::from([0xab; 32]);

        // ((51 puzzle_hash amount (hint)))
        let conditions = ((51u8, (ph, (1000u64, ((hint, ()), ())))), ())
            .to_clvm(&mut a)
            .unwrap();

        let solution_bytes = node_to_bytes(&a, conditions).unwrap();
        let puzzle_bytes = [0x01u8];
        let coin = Coin::new([0xcc; 32].into(), [0xdd; 32].into(), 1000);
        solution_generator([(coin, puzzle_bytes.as_slice(), solution_bytes.as_slice())]).unwrap()
    }

    #[rstest]
    #[case(1, true)]
    #[case(32, true)]
    #[case(33, false)]
    #[case(100_000, false)]
    fn test_hint_length_filtering(#[case] hint_size: usize, #[case] expect_hint: bool) {
        let no_blocks: &[&[u8]] = &[];
        let generator = make_create_coin_generator(hint_size);
        let (additions, _removals) = additions_and_removals(
            &generator,
            no_blocks,
            ConsensusFlags::empty(),
            &TEST_CONSTANTS,
        )
        .unwrap();
        assert_eq!(additions.len(), 1);
        assert_eq!(additions[0].1.is_some(), expect_hint);
    }

    #[test]
    fn test_pair_hint_is_ignored() {
        use crate::solution_generator::solution_generator;
        use clvm_traits::ToClvm;
        use clvmr::allocator::Allocator;
        use clvmr::serde::node_to_bytes;

        let mut a = Allocator::new();

        let pair_hint = (1u8, 2u8).to_clvm(&mut a).unwrap();
        let ph = Bytes32::from([0xab; 32]);

        // ((51 puzzle_hash amount ((1 . 2))))
        let conditions = ((51u8, (ph, (1000u64, ((pair_hint, ()), ())))), ())
            .to_clvm(&mut a)
            .unwrap();

        let solution_bytes = node_to_bytes(&a, conditions).unwrap();
        let puzzle_bytes = [0x01u8];
        let coin = Coin::new([0xcc; 32].into(), [0xdd; 32].into(), 1000);
        let generator =
            solution_generator([(coin, puzzle_bytes.as_slice(), solution_bytes.as_slice())])
                .unwrap();

        let no_blocks: &[&[u8]] = &[];
        let (additions, _removals) = additions_and_removals(
            &generator,
            no_blocks,
            ConsensusFlags::empty(),
            &TEST_CONSTANTS,
        )
        .unwrap();
        assert_eq!(additions.len(), 1);
        assert!(additions[0].1.is_none(), "pair hint should be ignored");
    }

    #[test]
    fn test_trusted_block_interned_spend_list_parity() {
        use crate::solution_generator::solution_generator_2026;

        let base_flags = ConsensusFlags::DONT_VALIDATE_SIGNATURE | ConsensusFlags::SIMPLE_GENERATOR;
        let interned_flags = base_flags | ConsensusFlags::INTERNED_SPEND_LIST;
        let blocks: &[&[u8]] = &[];

        let puzzle_hash = tree_hash_atom(&[1]).to_bytes();
        let empty_solution: &[u8] = &[0x80];
        let spends = [(
            Coin::new([0u8; 32].into(), puzzle_hash.into(), 0),
            IDENTITY_PUZZLE,
            empty_solution,
        )];
        let classic = solution_generator(spends).expect("solution_generator");
        let interned = solution_generator_2026(spends).expect("solution_generator_2026");

        let expected =
            get_coinspends_for_trusted_block(&TEST_CONSTANTS, &classic, blocks, base_flags)
                .expect("classic get_coinspends_for_trusted_block");
        let actual =
            get_coinspends_for_trusted_block(&TEST_CONSTANTS, &interned, blocks, interned_flags)
                .expect("interned get_coinspends_for_trusted_block");
        assert_eq!(actual.len(), 1);
        assert_eq!(expected, actual);

        let expected = get_coinspends_with_conditions_for_trusted_block(
            &TEST_CONSTANTS,
            &classic,
            blocks,
            base_flags,
        )
        .expect("classic get_coinspends_with_conditions_for_trusted_block");
        let actual = get_coinspends_with_conditions_for_trusted_block(
            &TEST_CONSTANTS,
            &interned,
            blocks,
            interned_flags,
        )
        .expect("interned get_coinspends_with_conditions_for_trusted_block");
        assert_eq!(actual.len(), 1);
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_additions_and_removals_interned_parity() {
        use crate::solution_generator::solution_generator_2026;
        use clvm_traits::ToClvm;
        use clvmr::allocator::Allocator;
        use clvmr::serde::node_to_bytes;

        let base_flags = ConsensusFlags::DONT_VALIDATE_SIGNATURE | ConsensusFlags::SIMPLE_GENERATOR;
        let interned_flags = base_flags | ConsensusFlags::INTERNED_SPEND_LIST;
        let blocks: &[&[u8]] = &[];

        // a spend whose identity puzzle emits CREATE_COIN with a hint, so the
        // additions path (not just removals) is exercised
        let puzzle_hash = Bytes32::from(tree_hash_atom(&[1]).to_bytes());
        let mut a = Allocator::new();
        let hint = a.new_atom(b"hint").unwrap();
        // the memos list's first element must itself be a list wrapping the
        // hint atom — (51 ph amount ((hint))) — a bare atom memo is ignored
        let cond = (CREATE_COIN, (puzzle_hash, (100u64, ((hint, 0), 0))))
            .to_clvm(&mut a)
            .unwrap();
        let conds = a.new_pair(cond, a.nil()).unwrap();
        let solution_bytes = node_to_bytes(&a, conds).unwrap();

        let coin = Coin::new([0u8; 32].into(), puzzle_hash, 100);
        let spends = [(coin, IDENTITY_PUZZLE, solution_bytes.as_slice())];
        let classic = solution_generator(spends).expect("solution_generator");
        let interned = solution_generator_2026(spends).expect("solution_generator_2026");

        let expected = additions_and_removals(&classic, blocks, base_flags, &TEST_CONSTANTS)
            .expect("classic additions_and_removals");
        let actual = additions_and_removals(&interned, blocks, interned_flags, &TEST_CONSTANTS)
            .expect("interned additions_and_removals");
        assert_eq!(expected, actual);

        // the parity assert above is only meaningful if this case actually
        // produced an addition carrying the hint
        let (additions, removals) = &actual;
        assert_eq!(removals.len(), 1);
        assert_eq!(removals[0].0, coin.coin_id());
        assert_eq!(additions.len(), 1);
        let (addition, addition_hint) = &additions[0];
        assert_eq!(addition.parent_coin_info, coin.coin_id());
        assert_eq!(addition.puzzle_hash, puzzle_hash);
        assert_eq!(addition.amount, 100);
        assert_eq!(
            addition_hint.as_ref().map(AsRef::as_ref),
            Some(&b"hint"[..])
        );
    }

    #[test]
    fn test_interned_spend_list_rejects_block_refs() {
        use crate::solution_generator::solution_generator_2026;

        let interned_flags = ConsensusFlags::DONT_VALIDATE_SIGNATURE
            | ConsensusFlags::SIMPLE_GENERATOR
            | ConsensusFlags::INTERNED_SPEND_LIST;
        let refs: &[&[u8]] = &[&[1]];

        let puzzle_hash = tree_hash_atom(&[1]).to_bytes();
        let empty_solution: &[u8] = &[0x80];
        let spends = [(
            Coin::new([0u8; 32].into(), puzzle_hash.into(), 0),
            IDENTITY_PUZZLE,
            empty_solution,
        )];
        let program = solution_generator_2026(spends).expect("solution_generator_2026");

        // INTERNED_SPEND_LIST disables generator references, with or without
        // SIMPLE_GENERATOR
        for flags in [
            interned_flags,
            ConsensusFlags::DONT_VALIDATE_SIGNATURE | ConsensusFlags::INTERNED_SPEND_LIST,
        ] {
            let result = run_block_generator2(
                &program,
                refs,
                u64::MAX,
                flags,
                &Signature::default(),
                None,
                &TEST_CONSTANTS,
            );
            assert_eq!(
                result.unwrap_err().error_code(),
                ErrorCode::TooManyGeneratorRefs
            );

            let result = get_coinspends_for_trusted_block(&TEST_CONSTANTS, &program, refs, flags);
            assert_eq!(
                result.unwrap_err().error_code(),
                ErrorCode::TooManyGeneratorRefs
            );

            let result = get_coinspends_with_conditions_for_trusted_block(
                &TEST_CONSTANTS,
                &program,
                refs,
                flags,
            );
            assert_eq!(
                result.unwrap_err().error_code(),
                ErrorCode::TooManyGeneratorRefs
            );

            let result = additions_and_removals(&program, refs, flags, &TEST_CONSTANTS);
            assert_eq!(
                result.unwrap_err().error_code(),
                ErrorCode::TooManyGeneratorRefs
            );
        }
    }
}
