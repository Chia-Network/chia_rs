use crate::consensus_constants::ConsensusConstants;
use crate::gen::conditions::{
    parse_spends, process_single_spend, validate_conditions, validate_signature, EmptyVisitor,
    ParseState, SpendBundleConditions,
};
use crate::gen::flags::{ALLOW_BACKREFS, DONT_VALIDATE_SIGNATURE};
use crate::gen::validation_error::{first, ErrorCode, ValidationErr};
use crate::generator_rom::{CLVM_DESERIALIZER, GENERATOR_ROM};
use chia_bls::{BlsCache, Signature};
use clvm_utils::{tree_hash_cached, TreeHash};
use clvmr::allocator::{Allocator, NodePtr};
use clvmr::chia_dialect::ChiaDialect;
use clvmr::cost::Cost;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;
use clvmr::serde::{node_from_bytes, node_from_bytes_backrefs, node_from_bytes_backrefs_record};
use std::collections::{HashMap, HashSet};

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

// Runs the generator ROM and passes in the program (transactions generator).
// The program is expected to return a list of spends. Each item being:

// (parent-coin-id puzzle-reveal amount solution)

// The puzzle-reveals are then executed with the corresponding solution being
// passed as the argument. The output from those puzzles are lists of
// conditions. The conditions are parsed and returned in the
// SpendBundleConditions. Some conditions are validated, and if invalid may
// cause the function to return an error.

// the only reason we need to pass in the allocator is because the returned
// SpendBundleConditions contains NodePtr fields. If that's changed, we could
// create the allocator inside this functions as well.
#[allow(clippy::too_many_arguments)]
pub fn run_block_generator<GenBuf: AsRef<[u8]>, I: IntoIterator<Item = GenBuf>>(
    a: &mut Allocator,
    program: &[u8],
    block_refs: I,
    max_cost: u64,
    flags: u32,
    signature: &Signature,
    bls_cache: Option<&mut BlsCache>,
    constants: &ConsensusConstants,
) -> Result<SpendBundleConditions, ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    let mut cost_left = max_cost;
    let byte_cost = program.len() as u64 * constants.cost_per_byte;

    subtract_cost(a, &mut cost_left, byte_cost)?;

    let generator_rom = node_from_bytes(a, &GENERATOR_ROM)?;
    let program = if (flags & ALLOW_BACKREFS) != 0 {
        node_from_bytes_backrefs(a, program)?
    } else {
        node_from_bytes(a, program)?
    };

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
        flags,
        signature,
        bls_cache,
        constants,
    )?;
    result.cost += max_cost - cost_left;
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

// This has the same behavior as run_block_generator() but implements the
// generator ROM in rust instead of using the CLVM implementation.
// it is not backwards compatible in the CLVM cost computation (in this version
// you only pay cost for the generator, the puzzles and the conditions).
// it also does not apply the stack depth or object allocation limits the same,
// as each puzzle run in its own environment.
#[allow(clippy::too_many_arguments)]
pub fn run_block_generator2<GenBuf: AsRef<[u8]>, I: IntoIterator<Item = GenBuf>>(
    a: &mut Allocator,
    program: &[u8],
    block_refs: I,
    max_cost: u64,
    flags: u32,
    signature: &Signature,
    bls_cache: Option<&mut BlsCache>,
    constants: &ConsensusConstants,
) -> Result<SpendBundleConditions, ValidationErr>
where
    <I as IntoIterator>::IntoIter: DoubleEndedIterator,
{
    let byte_cost = program.len() as u64 * constants.cost_per_byte;

    let mut cost_left = max_cost;
    subtract_cost(a, &mut cost_left, byte_cost)?;

    let (program, backrefs) = if (flags & ALLOW_BACKREFS) != 0 {
        node_from_bytes_backrefs_record(a, program)?
    } else {
        (node_from_bytes(a, program)?, HashSet::<NodePtr>::new())
    };

    let args = setup_generator_args(a, block_refs)?;
    let dialect = ChiaDialect::new(flags);

    let Reduction(clvm_cost, mut all_spends) = run_program(a, &dialect, program, args, cost_left)?;

    subtract_cost(a, &mut cost_left, clvm_cost)?;
    all_spends = first(a, all_spends)?;

    // at this point all_spends is a list of:
    // (parent-coin-id puzzle-reveal amount solution . extra)
    // where extra may be nil, or additional extension data

    let mut ret = SpendBundleConditions::default();
    let mut state = ParseState::default();
    let mut cache = HashMap::<NodePtr, TreeHash>::new();

    while let Some((spend, rest)) = a.next(all_spends) {
        all_spends = rest;
        // process the spend
        let [parent_id, puzzle, amount, solution, _spend_level_extra] =
            extract_n::<5>(a, spend, ErrorCode::InvalidCondition)?;

        let Reduction(clvm_cost, conditions) =
            run_program(a, &dialect, puzzle, solution, cost_left)?;

        subtract_cost(a, &mut cost_left, clvm_cost)?;

        let buf = tree_hash_cached(a, puzzle, &backrefs, &mut cache);
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
            constants,
        )?;
    }
    if a.atom_len(all_spends) != 0 {
        return Err(ValidationErr(all_spends, ErrorCode::GeneratorRuntimeError));
    }

    validate_conditions(a, &ret, &state, a.nil(), flags)?;
    validate_signature(&state, signature, flags, bls_cache)?;
    ret.validated_signature = (flags & DONT_VALIDATE_SIGNATURE) == 0;

    ret.cost = max_cost - cost_left;
    Ok(ret)
}
