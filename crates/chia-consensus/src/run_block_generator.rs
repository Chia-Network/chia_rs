use crate::allocator::make_allocator;
use crate::condition_sanitizers::parse_amount;
use crate::conditions::{
    parse_spends, process_single_spend, validate_conditions, validate_signature, EmptyVisitor,
    ParseState, SpendBundleConditions,
};
use crate::consensus_constants::ConsensusConstants;
use crate::flags::DONT_VALIDATE_SIGNATURE;
use crate::generator_rom::{CLVM_DESERIALIZER, GENERATOR_ROM};
use crate::validation_error::{first, ErrorCode, ValidationErr};
use chia_bls::{BlsCache, Signature};
use chia_protocol::{BytesImpl, Coin, CoinSpend, Program};
use clvm_traits::FromClvm;
use clvm_utils::{tree_hash_cached, TreeCache};
use clvmr::allocator::{Allocator, NodePtr};
use clvmr::chia_dialect::ChiaDialect;
use clvmr::cost::Cost;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;
use clvmr::serde::{node_from_bytes, node_from_bytes_backrefs};
use clvmr::{SExp, LIMIT_HEAP};

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
) -> Result<NodePtr, ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    let clvm_deserializer = node_from_bytes(a, &CLVM_DESERIALIZER)?;

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
/// the only reason we need to pass in the allocator is because the returned
/// SpendBundleConditions contains NodePtr fields. If that's changed, we could
/// create the allocator inside this functions as well.
#[allow(clippy::too_many_arguments)]
pub fn run_block_generator<GenBuf: AsRef<[u8]>, I: IntoIterator<Item = GenBuf>>(
    a: &mut Allocator,
    program: &[u8],
    block_refs: I,
    max_cost: u64,
    flags: u32,
    signature: &Signature,
    bls_cache: Option<&BlsCache>,
    constants: &ConsensusConstants,
) -> Result<SpendBundleConditions, ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    let mut cost_left = max_cost;
    let byte_cost = program.len() as u64 * constants.cost_per_byte;

    subtract_cost(a, &mut cost_left, byte_cost)?;

    let generator_rom = node_from_bytes(a, &GENERATOR_ROM)?;
    let program = node_from_bytes_backrefs(a, program)?;

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

    let dialect = ChiaDialect::new(flags);
    let Reduction(clvm_cost, generator_output) =
        run_program(a, &dialect, generator_rom, args, cost_left)?;

    subtract_cost(a, &mut cost_left, clvm_cost)?;

    // we pass in what's left of max_cost here, to fail early in case the
    // cost of a condition brings us over the cost limit
    let mut result = parse_spends::<EmptyVisitor>(
        a,
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
    Ok(result)
}

pub fn extract_n<const N: usize>(
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

/// This has the same behavior as run_block_generator() but implements the
/// generator ROM in rust instead of using the CLVM implementation.
/// it is not backwards compatible in the CLVM cost computation (in this version
/// you only pay cost for the generator, the puzzles and the conditions).
/// it also does not apply the stack depth or object allocation limits the same,
/// as each puzzle run in its own environment.
#[allow(clippy::too_many_arguments)]
pub fn run_block_generator2<GenBuf: AsRef<[u8]>, I: IntoIterator<Item = GenBuf>>(
    a: &mut Allocator,
    program: &[u8],
    block_refs: I,
    max_cost: u64,
    flags: u32,
    signature: &Signature,
    bls_cache: Option<&BlsCache>,
    constants: &ConsensusConstants,
) -> Result<SpendBundleConditions, ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    let byte_cost = program.len() as u64 * constants.cost_per_byte;

    let mut cost_left = max_cost;
    subtract_cost(a, &mut cost_left, byte_cost)?;

    let program = node_from_bytes_backrefs(a, program)?;

    let args = setup_generator_args(a, block_refs)?;
    let dialect = ChiaDialect::new(flags);

    let Reduction(clvm_cost, all_spends) = run_program(a, &dialect, program, args, cost_left)?;

    subtract_cost(a, &mut cost_left, clvm_cost)?;

    let mut ret = SpendBundleConditions::default();

    let all_spends = first(a, all_spends)?;
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
        let [_, puzzle, _] = extract_n::<3>(a, spend, ErrorCode::InvalidCondition)?;
        cache.visit_tree(a, puzzle);
    }

    let mut iter = all_spends;
    while let Some((spend, rest)) = a.next(iter) {
        iter = rest;
        // process the spend
        let [parent_id, puzzle, amount, solution, _spend_level_extra] =
            extract_n::<5>(a, spend, ErrorCode::InvalidCondition)?;

        let Reduction(clvm_cost, conditions) =
            run_program(a, &dialect, puzzle, solution, cost_left)?;

        subtract_cost(a, &mut cost_left, clvm_cost)?;
        ret.execution_cost += clvm_cost;

        let buf = tree_hash_cached(a, puzzle, &mut cache);
        let puzzle_hash = a.new_atom(&buf)?;

        process_single_spend::<EmptyVisitor>(
            a,
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

    validate_conditions(a, &ret, &state, a.nil(), flags)?;
    validate_signature(&state, signature, flags, bls_cache)?;
    ret.validated_signature = (flags & DONT_VALIDATE_SIGNATURE) == 0;

    ret.cost = max_cost - cost_left;
    Ok(ret)
}

pub fn get_coinspends_for_trusted_block<GenBuf: AsRef<[u8]>, I: IntoIterator<Item = GenBuf>>(
    constants: &ConsensusConstants,
    generator: &Program,
    refs: I,
    flags: u32,
) -> Result<Vec<CoinSpend>, ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    let mut a = make_allocator(LIMIT_HEAP);
    let mut output = Vec::<CoinSpend>::new();

    let program = node_from_bytes_backrefs(&mut a, generator)?;
    let args = setup_generator_args(&mut a, refs)?;
    let dialect = ChiaDialect::new(flags);

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
        let Ok(puzzle_program) = Program::from_clvm(&a, puzzle) else {
            continue;
        };
        let Ok(solution_program) = Program::from_clvm(&a, solution) else {
            continue;
        };
        let coinspend = CoinSpend::new(coin, puzzle_program, solution_program);
        output.push(coinspend);
    }
    Ok(output)
}

// this function returns a list of tuples (coinspend, conditions)
// conditions are formatted as a vec of tuples of (condition_opcode, args)
#[allow(clippy::type_complexity)]
pub fn get_coinspends_with_conditions_for_trusted_block<
    GenBuf: AsRef<[u8]>,
    I: IntoIterator<Item = GenBuf>,
>(
    constants: &ConsensusConstants,
    generator: &Program,
    refs: I,
    flags: u32,
) -> Result<Vec<(CoinSpend, Vec<(u32, Vec<Vec<u8>>)>)>, ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    let mut a = make_allocator(LIMIT_HEAP);
    let mut output = Vec::<(CoinSpend, Vec<(u32, Vec<Vec<u8>>)>)>::new();

    let program = node_from_bytes_backrefs(&mut a, generator)?;
    let args = setup_generator_args(&mut a, refs)?;
    let dialect = ChiaDialect::new(flags);

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
            u64::from_clvm(&a, amount)
                .map_err(|_| ValidationErr(first, ErrorCode::InvalidCoinAmount))?,
        );
        let Ok(puzzle_program) = Program::from_clvm(&a, puzzle) else {
            continue;
        };
        let Ok(solution_program) = Program::from_clvm(&a, solution) else {
            continue;
        };
        let coinspend = CoinSpend::new(coin, puzzle_program.clone(), solution_program.clone());
        let Ok((_, res)) =
            puzzle_program.run(&mut a, flags, constants.max_block_cost_clvm, &solution)
        else {
            output.push((coinspend, cond_output));
            continue; // Skip this spend on error
        };
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
                    match a.sexp(condition_values) {
                        SExp::Atom => {
                            // a reasonable max length of an atom is 1,024 bytes
                            if a.atom_len(condition_values) >= 1024 {
                                // skip this condition
                                continue 'outer;
                            }
                            let bytes = a.atom(condition_values).to_vec();
                            bytes_vec.push(bytes);
                        }
                        SExp::Pair(..) => continue, // ignore lists in condition args
                    };
                } else {
                    break 'inner; // we only care about the first 5 condition arguments
                }
            }

            // we have a valid condition
            cond_output.push((opcode, bytes_vec));
        }
        output.push((coinspend, cond_output));
    }
    Ok(output)
}
