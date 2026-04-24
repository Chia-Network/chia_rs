use crate::allocator::make_allocator;
use crate::condition_sanitizers::parse_amount;
use crate::conditions::{
    EmptyVisitor, MAX_SPENDS_PER_BLOCK, ParseState, SpendBundleConditions, parse_spends,
    process_single_spend, validate_conditions, validate_signature,
};
use crate::consensus_constants::ConsensusConstants;
use crate::flags::ConsensusFlags;
use crate::generator_cost::total_cost_from_tree;
use crate::opcodes::{
    AGG_SIG_AMOUNT, AGG_SIG_ME, AGG_SIG_PARENT, AGG_SIG_PARENT_AMOUNT, AGG_SIG_PARENT_PUZZLE,
    AGG_SIG_PUZZLE, AGG_SIG_PUZZLE_AMOUNT, AGG_SIG_UNSAFE, CREATE_COIN,
};
use crate::validation_error::{ErrorCode, ValidationErr, first};
use chia_bls::{BlsCache, Signature};
use chia_protocol::{BytesImpl, Coin, CoinSpend, Program};
use chia_puzzles::{CHIALISP_DESERIALISATION, ROM_BOOTSTRAP_GENERATOR};
use clvm_traits::FromClvm;
use clvm_traits::MatchByte;
use clvm_utils::{TreeCache, tree_hash_cached};
use clvmr::SExp;
use clvmr::allocator::{Allocator, NodePtr};
use clvmr::chia_dialect::ChiaDialect;
use clvmr::cost::Cost;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;
use clvmr::serde::{
    InternedTree, intern, node_from_bytes, node_from_bytes_auto, node_from_bytes_backrefs,
};

pub fn subtract_cost(
    a: &Allocator,
    cost_left: &mut Cost,
    subtract: Cost,
) -> Result<(), ValidationErr> {
    if subtract > *cost_left {
        Err(ValidationErr(a.nil(), ErrorCode::CostExceeded))
    } else {
        *cost_left -= subtract;
        Ok(())
    }
}

/// Prepares the arguments passed to the block generator. They are in the form:
/// (DESERIALIZER_MOD (block1 block2 block3 ...))
pub fn setup_generator_args<GenBuf: AsRef<[u8]>, I: IntoIterator<Item = GenBuf>>(
    a: &mut Allocator,
    block_refs: I,
    flags: ConsensusFlags,
) -> Result<NodePtr, ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    // once we have soft-forked in requiring simple generators, we no longer
    // need to pass in the deserialization program
    if flags.contains(ConsensusFlags::SIMPLE_GENERATOR) {
        if block_refs.into_iter().next().is_some() {
            return Err(ValidationErr(a.nil(), ErrorCode::TooManyGeneratorRefs));
        }
        return Ok(a.nil());
    }
    let clvm_deserializer = node_from_bytes(a, &CHIALISP_DESERIALISATION)?;

    // iterate in reverse order since we're building a linked list from
    // the tail
    let mut blocks = NodePtr::NIL;
    for g in block_refs.into_iter().rev() {
        let ref_gen = a.new_atom(g.as_ref())?;
        blocks = a.new_pair(ref_gen, blocks)?;
    }

    // the first argument to the generator is the serializer, followed by a list
    // of the blocks it requested.
    let args = a.new_pair(blocks, NodePtr::NIL)?;
    Ok(a.new_pair(clvm_deserializer, args)?)
}

/// Runs the generator ROM and passes in the program (transactions generator).
/// The program is expected to return a list of spends. Each item being:
///
/// (parent-coin-id puzzle-reveal amount solution)
///
/// The puzzle-reveals are then executed with the corresponding solution being
/// passed as the argument. The output from those puzzles are lists of
/// conditions. The conditions are parsed and returned in the
/// SpendBundleConditions. Some conditions are validated, and if invalid may
/// cause the function to return an error.
///
/// Creates an allocator internally based on the consensus flags (using
/// `make_allocator(flags)`). Returns `(Allocator, SpendBundleConditions)` since
/// the conditions contain NodePtr references into the allocator.
#[allow(clippy::too_many_arguments)]
pub fn run_block_generator<GenBuf: AsRef<[u8]>, I: IntoIterator<Item = GenBuf>>(
    program: &[u8],
    block_refs: I,
    max_cost: u64,
    flags: ConsensusFlags,
    signature: &Signature,
    bls_cache: Option<&BlsCache>,
    constants: &ConsensusConstants,
) -> Result<(Allocator, SpendBundleConditions), ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    check_generator_quote(program, flags)?;
    let mut a = make_allocator(flags);
    let mut cost_left = max_cost;
    let byte_cost = program.len() as u64 * constants.cost_per_byte;

    subtract_cost(&a, &mut cost_left, byte_cost)?;

    let rom_generator = node_from_bytes(&mut a, &ROM_BOOTSTRAP_GENERATOR)?;
    let program = node_from_bytes_backrefs(&mut a, program)?;
    check_generator_node(&a, program, flags)?;

    // this is setting up the arguments to be passed to the generator ROM,
    // not the actual generator (the ROM does that).
    // iterate in reverse order since we're building a linked list from
    // the tail
    let mut args = a.nil();
    for g in block_refs.into_iter().rev() {
        let ref_gen = a.new_atom(g.as_ref())?;
        args = a.new_pair(ref_gen, args)?;
    }

    args = a.new_pair(args, a.nil())?;
    let args = a.new_pair(args, a.nil())?;
    let args = a.new_pair(program, args)?;

    let dialect = ChiaDialect::new(flags.to_clvm_flags());
    let Reduction(clvm_cost, generator_output) =
        run_program(&mut a, &dialect, rom_generator, args, cost_left)?;

    subtract_cost(&a, &mut cost_left, clvm_cost)?;

    // we pass in what's left of max_cost here, to fail early in case the
    // cost of a condition brings us over the cost limit
    let mut result = parse_spends::<EmptyVisitor>(
        &a,
        generator_output,
        cost_left,
        0, // clvm_cost is not known per puzzle pre-hard fork
        flags,
        signature,
        bls_cache,
        constants,
    )?;
    result.cost += max_cost - cost_left;
    result.execution_cost = clvm_cost;
    Ok((a, result))
}

fn extract_n<const N: usize>(
    a: &Allocator,
    mut n: NodePtr,
    e: ErrorCode,
) -> Result<[NodePtr; N], ValidationErr> {
    let mut ret: [NodePtr; N] = [NodePtr::NIL; N];
    let mut counter = 0;
    assert!(N > 0);
    while let Some((item, rest)) = a.next(n) {
        if counter == N - 1 {
            break;
        }
        n = rest;
        ret[counter] = item;
        counter += 1;
    }
    if counter != N - 1 {
        return Err(ValidationErr(n, e));
    }
    ret[counter] = n;
    Ok(ret)
}

// this function checks if the generator start with a quote
// this is required after the SIMPLE_GENERATOR fork is active
#[inline]
pub fn check_generator_quote(program: &[u8], flags: ConsensusFlags) -> Result<(), ValidationErr> {
    if flags.contains(ConsensusFlags::INTERNED_GENERATOR) {
        // serde_2026 blocks have a different serialization header
        return Ok(());
    }
    if !flags.contains(ConsensusFlags::SIMPLE_GENERATOR) || program.starts_with(&[0xff, 0x01]) {
        Ok(())
    } else {
        Err(ValidationErr(
            NodePtr::NIL,
            ErrorCode::ComplexGeneratorReceived,
        ))
    }
}

// this function is mostly the same as above but is a double check in case of
// discrepancies in serialized vs deserialized forms
#[inline]
pub fn check_generator_node(
    a: &Allocator,
    program: NodePtr,
    flags: ConsensusFlags,
) -> Result<(), ValidationErr> {
    if !flags.contains(ConsensusFlags::SIMPLE_GENERATOR)
        || flags.contains(ConsensusFlags::INTERNED_GENERATOR)
    {
        return Ok(());
    }
    // this expects an atom with a single byte value of 1 as the first value in the list
    match <(MatchByte<1>, NodePtr)>::from_clvm(a, program) {
        Err(..) => Err(ValidationErr(
            NodePtr::NIL,
            ErrorCode::ComplexGeneratorReceived,
        )),
        _ => Ok(()),
    }
}

/// This has the same behavior as run_block_generator() but implements the
/// generator ROM in rust instead of using the CLVM implementation.
/// it is not backwards compatible in the CLVM cost computation (in this version
/// you only pay cost for the generator, the puzzles and the conditions).
/// it also does not apply the stack depth or object allocation limits the same,
/// as each puzzle run in its own environment.
///
/// Creates an allocator internally based on the consensus flags (using
/// `make_allocator(flags)`). Returns `(Allocator, SpendBundleConditions)` since
/// the conditions contain NodePtr references into the allocator.
#[allow(clippy::too_many_arguments)]
pub fn run_block_generator2<GenBuf: AsRef<[u8]>, I: IntoIterator<Item = GenBuf>>(
    program: &[u8],
    block_refs: I,
    max_cost: u64,
    flags: ConsensusFlags,
    signature: &Signature,
    bls_cache: Option<&BlsCache>,
    constants: &ConsensusConstants,
) -> Result<(Allocator, SpendBundleConditions), ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    check_generator_quote(program, flags)?;

    let (mut a, base_cost, program) = if flags.contains(ConsensusFlags::INTERNED_GENERATOR) {
        let mut decode_allocator = Allocator::new();
        let program_node = node_from_bytes_auto(&mut decode_allocator, program)
            .map_err(|_| ValidationErr(NodePtr::NIL, ErrorCode::GeneratorRuntimeError))?;
        let interned = intern(&decode_allocator, program_node)
            .map_err(|_| ValidationErr(NodePtr::NIL, ErrorCode::GeneratorRuntimeError))?;
        let cost = total_cost_from_tree(&interned);
        let InternedTree {
            allocator, root, ..
        } = interned;
        drop(decode_allocator);
        (allocator, cost, root)
    } else {
        let mut a = make_allocator(flags);
        let byte_cost = program.len() as u64 * constants.cost_per_byte;
        let program = node_from_bytes_backrefs(&mut a, program)?;
        (a, byte_cost, program)
    };

    let mut cost_left = max_cost;
    subtract_cost(&a, &mut cost_left, base_cost)?;

    check_generator_node(&a, program, flags)?;

    let args = setup_generator_args(&mut a, block_refs, flags)?;
    let dialect = ChiaDialect::new(flags.to_clvm_flags());

    let Reduction(clvm_cost, all_spends) = run_program(&mut a, &dialect, program, args, cost_left)?;

    subtract_cost(&a, &mut cost_left, clvm_cost)?;

    let mut ret = SpendBundleConditions::default();

    let all_spends = first(&a, all_spends)?;
    ret.execution_cost += clvm_cost;

    // at this point all_spends is a list of:
    // (parent-coin-id puzzle-reveal amount solution . extra)
    // where extra may be nil, or additional extension data

    let mut state = ParseState::default();
    let mut cache = TreeCache::default();

    // first iterate over all puzzle reveals to find duplicate nodes, to know
    // what to memoize during tree hash computations. This is managed by
    // TreeCache
    let mut iter = all_spends;
    while let Some((spend, rest)) = a.next(iter) {
        iter = rest;
        let [_, puzzle, _] = extract_n::<3>(&a, spend, ErrorCode::InvalidCondition)?;
        cache.visit_tree(&a, puzzle);
    }

    let mut spends_left: usize = if flags.contains(ConsensusFlags::LIMIT_SPENDS) {
        MAX_SPENDS_PER_BLOCK
    } else {
        usize::MAX
    };

    let mut iter = all_spends;
    while let Some((spend, rest)) = a.next(iter) {
        iter = rest;
        if spends_left == 0 {
            return Err(ValidationErr(spend, ErrorCode::TooManySpends));
        }
        spends_left -= 1;
        // process the spend
        let [parent_id, puzzle, amount, solution, _spend_level_extra] =
            extract_n::<5>(&a, spend, ErrorCode::InvalidCondition)?;

        let Reduction(clvm_cost, conditions) =
            run_program(&mut a, &dialect, puzzle, solution, cost_left)?;

        subtract_cost(&a, &mut cost_left, clvm_cost)?;
        ret.execution_cost += clvm_cost;

        let buf = tree_hash_cached(&a, puzzle, &mut cache);
        let puzzle_hash = a.new_atom(&buf)?;

        process_single_spend::<EmptyVisitor>(
            &a,
            &mut ret,
            &mut state,
            parent_id,
            puzzle_hash,
            amount,
            conditions,
            flags,
            &mut cost_left,
            clvm_cost,
            constants,
        )?;
    }
    if a.atom_len(iter) != 0 {
        return Err(ValidationErr(iter, ErrorCode::GeneratorRuntimeError));
    }

    validate_conditions(&a, &ret, &state, a.nil(), flags)?;
    validate_signature(&state, signature, flags, bls_cache)?;
    ret.validated_signature = !flags.contains(ConsensusFlags::DONT_VALIDATE_SIGNATURE);

    ret.cost = max_cost - cost_left;
    Ok((a, ret))
}

// this function is less capable of handling problematic generators as they are
// returning serialized puzzles, which may not be possible. They will simply ignore many of the bad cases.
pub fn get_coinspends_for_trusted_block<GenBuf: AsRef<[u8]>, I: IntoIterator<Item = GenBuf>>(
    constants: &ConsensusConstants,
    generator: &Program,
    refs: I,
    flags: ConsensusFlags,
) -> Result<Vec<CoinSpend>, ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    let mut a = make_allocator(flags);
    check_generator_quote(generator.as_ref(), flags)?;
    let mut output = Vec::<CoinSpend>::new();

    let program = node_from_bytes_auto(&mut a, generator)?;
    check_generator_node(&a, program, flags)?;
    let args = setup_generator_args(&mut a, refs, flags)?;
    let dialect = ChiaDialect::new(flags.to_clvm_flags());

    let Reduction(_clvm_cost, res) = run_program(
        &mut a,
        &dialect,
        program,
        args,
        constants.max_block_cost_clvm,
    )?;

    let (first, _rest) = a
        .next(res)
        .ok_or(ValidationErr(res, ErrorCode::GeneratorRuntimeError))?;
    let mut cache = TreeCache::default();
    let mut iter = first;
    while let Some((spend, rest)) = a.next(iter) {
        iter = rest;
        let Ok([_, puzzle, _]) = extract_n::<3>(&a, spend, ErrorCode::InvalidCondition) else {
            continue;
        };
        cache.visit_tree(&a, puzzle);
    }
    iter = first;
    while let Some((spend, rest)) = a.next(iter) {
        iter = rest;
        let Ok([parent_id, puzzle, amount, solution, _spend_level_extra]) =
            extract_n::<5>(&a, spend, ErrorCode::InvalidCondition)
        else {
            continue; // if we fail at this step then maybe the generator was malicious - try other spends
        };
        let puzhash = tree_hash_cached(&a, puzzle, &mut cache);
        let parent_id = BytesImpl::<32>::from_clvm(&a, parent_id)
            .map_err(|_| ValidationErr(first, ErrorCode::InvalidParentId))?;
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
    generator: &Program,
    refs: I,
    flags: ConsensusFlags,
) -> Result<Vec<(CoinSpend, Vec<(u32, Vec<Vec<u8>>)>)>, ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    let mut a = make_allocator(flags);
    check_generator_quote(generator.as_ref(), flags)?;
    let mut output = Vec::<(CoinSpend, Vec<(u32, Vec<Vec<u8>>)>)>::new();

    let program = node_from_bytes_auto(&mut a, generator)?;
    check_generator_node(&a, program, flags)?;
    let args = setup_generator_args(&mut a, refs, flags)?;
    let dialect = ChiaDialect::new(flags.to_clvm_flags());

    let Reduction(_clvm_cost, res) = run_program(
        &mut a,
        &dialect,
        program,
        args,
        constants.max_block_cost_clvm,
    )
    .map_err(|_| ValidationErr(program, ErrorCode::GeneratorRuntimeError))?;

    let (first, _rest) = a
        .next(res)
        .ok_or(ValidationErr(res, ErrorCode::GeneratorRuntimeError))?;
    let mut cache = TreeCache::default();
    let mut iter = first;
    while let Some((spend, rest)) = a.next(iter) {
        iter = rest;
        let [_, puzzle, _] = extract_n::<3>(&a, spend, ErrorCode::InvalidCondition)?;
        cache.visit_tree(&a, puzzle);
    }
    iter = first;
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
            .map_err(|_| ValidationErr(first, ErrorCode::InvalidParentId))?;
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
        .map_err(|_| ValidationErr(program, ErrorCode::GeneratorRuntimeError))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conditions::MAX_SPENDS_PER_BLOCK;
    use crate::consensus_constants::TEST_CONSTANTS;
    use crate::opcodes::{CREATE_COIN, CREATE_COIN_COST, NEW_CREATE_COIN_COST, SPEND_COST};
    use crate::solution_generator::solution_generator;
    use chia_protocol::Bytes32;
    use clvm_traits::ToClvm;
    use clvm_utils::tree_hash_atom;
    use clvmr::serde::node_to_bytes;
    use rstest::rstest;

    const IDENTITY_PUZZLE: &[u8] = &[1];

    fn make_generator(num_spends: usize) -> Vec<u8> {
        let puzzle_hash = tree_hash_atom(&[1]).to_bytes();
        let empty_solution: &[u8] = &[0x80]; // serialized nil

        let spends = (0..num_spends).map(|i| {
            let mut parent = [0u8; 32];
            parent[0..4].copy_from_slice(&(i as u32).to_be_bytes());
            (
                Coin::new(parent.into(), puzzle_hash.into(), 0),
                IDENTITY_PUZZLE,
                empty_solution,
            )
        });

        solution_generator(spends).expect("solution_generator")
    }

    fn make_generator_with_create_coins(num_spends: usize, coins_per_spend: usize) -> Vec<u8> {
        let puzzle_hash = Bytes32::from(tree_hash_atom(&[1]).to_bytes());

        let mut a = Allocator::new();
        let mut conds = a.nil();
        for i in 0..coins_per_spend {
            let cond = (CREATE_COIN, (puzzle_hash, (i as u64, 0)))
                .to_clvm(&mut a)
                .unwrap();
            conds = a.new_pair(cond, conds).unwrap();
        }
        let solution_bytes = node_to_bytes(&a, conds).unwrap();

        let total_amount: u64 = (0..coins_per_spend as u64).sum();
        let spends = (0..num_spends).map(|i| {
            let mut parent = [0u8; 32];
            parent[0..4].copy_from_slice(&(i as u32).to_be_bytes());
            (
                Coin::new(parent.into(), puzzle_hash, total_amount),
                IDENTITY_PUZZLE,
                solution_bytes.as_slice(),
            )
        });

        solution_generator(spends).expect("solution_generator")
    }

    #[rstest]
    #[case(MAX_SPENDS_PER_BLOCK, ConsensusFlags::LIMIT_SPENDS, None)]
    #[case(MAX_SPENDS_PER_BLOCK + 1, ConsensusFlags::LIMIT_SPENDS, Some(ErrorCode::TooManySpends))]
    #[case(MAX_SPENDS_PER_BLOCK + 1, ConsensusFlags::empty(), None)]
    fn test_limit_spends_run_block_generator2(
        #[case] num_spends: usize,
        #[case] extra_flags: ConsensusFlags,
        #[case] expected_err: Option<ErrorCode>,
    ) {
        let program = make_generator(num_spends);
        let flags = extra_flags | ConsensusFlags::DONT_VALIDATE_SIGNATURE;
        let blocks: &[&[u8]] = &[];
        let result = run_block_generator2(
            &program,
            blocks,
            u64::MAX,
            flags,
            &Signature::default(),
            None,
            &TEST_CONSTANTS,
        );
        match (expected_err, result) {
            (Some(err), Err(e)) => {
                assert_eq!(e.1, err);
            }
            (None, Ok(conds)) => {
                assert_eq!(conds.1.spends.len(), num_spends);
            }
            _ => {
                panic!("mismatch");
            }
        }
    }

    #[rstest]
    #[case(1, 1)]
    #[case(3, 1)]
    #[case(1, 3)]
    #[case(5, 5)]
    fn test_cost_conditions_with_create_coin(
        #[case] num_spends: usize,
        #[case] coins_per_spend: usize,
    ) {
        let program = make_generator_with_create_coins(num_spends, coins_per_spend);
        let blocks: &[&[u8]] = &[];
        let num_coins = (num_spends * coins_per_spend) as u64;

        let (_, without) = run_block_generator2(
            &program,
            blocks,
            u64::MAX,
            ConsensusFlags::DONT_VALIDATE_SIGNATURE,
            &Signature::default(),
            None,
            &TEST_CONSTANTS,
        )
        .expect("without COST_CONDITIONS");

        let (_, with) = run_block_generator2(
            &program,
            blocks,
            u64::MAX,
            ConsensusFlags::DONT_VALIDATE_SIGNATURE | ConsensusFlags::COST_CONDITIONS,
            &Signature::default(),
            None,
            &TEST_CONSTANTS,
        )
        .expect("with COST_CONDITIONS");

        assert_eq!(without.spends.len(), num_spends);
        assert_eq!(with.spends.len(), num_spends);

        assert_eq!(without.condition_cost, CREATE_COIN_COST * num_coins);
        assert_eq!(
            with.condition_cost,
            SPEND_COST * num_spends as u64 + NEW_CREATE_COIN_COST * num_coins
        );

        assert_eq!(without.execution_cost, with.execution_cost);
    }

    #[test]
    fn test_check_generator_quote_simple_only_rejects_non_quote() {
        let flags = ConsensusFlags::SIMPLE_GENERATOR;
        // Non-quote generator: starts with 0x80 (nil atom), not [0xff, 0x01]
        assert_eq!(
            check_generator_quote(&[0x80], flags).unwrap_err().1,
            ErrorCode::ComplexGeneratorReceived,
        );
        // Valid quote generator: starts with [0xff, 0x01]
        assert!(check_generator_quote(&[0xff, 0x01, 0x80], flags).is_ok());
    }

    #[test]
    fn test_check_generator_quote_interned_bypasses_check() {
        let flags = ConsensusFlags::SIMPLE_GENERATOR | ConsensusFlags::INTERNED_GENERATOR;
        // With INTERNED_GENERATOR, even non-quote bytes are accepted
        assert!(check_generator_quote(&[0x80], flags).is_ok());
        assert!(check_generator_quote(&[0x00, 0x42], flags).is_ok());
    }

    #[test]
    fn test_check_generator_quote_pre_simple_always_passes() {
        let flags = ConsensusFlags::empty();
        // Before SIMPLE_GENERATOR, anything is accepted
        assert!(check_generator_quote(&[0x80], flags).is_ok());
        assert!(check_generator_quote(&[0xff, 0x01, 0x80], flags).is_ok());
    }

    #[test]
    fn test_check_generator_node_simple_only_rejects_non_quote() {
        let flags = ConsensusFlags::SIMPLE_GENERATOR;
        let mut a = Allocator::new();
        // Build a non-quote tree: just an atom (not (1 . rest))
        let atom = a.new_atom(&[42]).unwrap();
        assert_eq!(
            check_generator_node(&a, atom, flags).unwrap_err().1,
            ErrorCode::ComplexGeneratorReceived,
        );
        // Build a valid (1 . nil) tree
        let one = a.new_atom(&[1]).unwrap();
        let nil = a.nil();
        let pair = a.new_pair(one, nil).unwrap();
        assert!(check_generator_node(&a, pair, flags).is_ok());
    }

    #[test]
    fn test_check_generator_node_interned_bypasses_check() {
        let flags = ConsensusFlags::SIMPLE_GENERATOR | ConsensusFlags::INTERNED_GENERATOR;
        let mut a = Allocator::new();
        let atom = a.new_atom(&[42]).unwrap();
        // INTERNED_GENERATOR bypasses the node check even for non-quote trees
        assert!(check_generator_node(&a, atom, flags).is_ok());
    }
}
