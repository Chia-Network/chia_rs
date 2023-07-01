use crate::gen::conditions::{
    parse_spends, process_single_spend, validate_conditions, ParseState, SpendBundleConditions,
};
use crate::gen::flags::ALLOW_BACKREFS;
use crate::gen::validation_error::{first, ErrorCode, ValidationErr};
use crate::generator_rom::{CLVM_DESERIALIZER, COST_PER_BYTE, GENERATOR_ROM};
use clvm_utils::tree_hash::tree_hash;
use clvmr::allocator::{Allocator, NodePtr};
use clvmr::chia_dialect::ChiaDialect;
use clvmr::cost::Cost;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;
use clvmr::serde::{node_from_bytes, node_from_bytes_backrefs};

fn subtract_cost(a: &Allocator, cost_left: &mut Cost, subtract: Cost) -> Result<(), ValidationErr> {
    if subtract > *cost_left {
        Err(ValidationErr(a.null(), ErrorCode::CostExceeded))
    } else {
        *cost_left -= subtract;
        Ok(())
    }
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
pub fn run_block_generator<GenBuf: AsRef<[u8]>>(
    a: &mut Allocator,
    program: &[u8],
    block_refs: &[GenBuf],
    max_cost: u64,
    flags: u32,
) -> Result<SpendBundleConditions, ValidationErr> {
    let mut cost_left = max_cost;
    let byte_cost = program.len() as u64 * COST_PER_BYTE;

    subtract_cost(a, &mut cost_left, byte_cost)?;

    let generator_rom = node_from_bytes(a, &GENERATOR_ROM)?;
    let program = if (flags & ALLOW_BACKREFS) != 0 {
        node_from_bytes_backrefs(a, program)?
    } else {
        node_from_bytes(a, program)?
    };

    // iterate in reverse order since we're building a linked list from
    // the tail
    let mut args = a.null();
    for g in block_refs.iter().rev() {
        let ref_gen = a.new_atom(g.as_ref())?;
        args = a.new_pair(ref_gen, args)?;
    }

    args = a.new_pair(args, a.null())?;
    let args = a.new_pair(args, a.null())?;
    let args = a.new_pair(program, args)?;

    let dialect = ChiaDialect::new(flags);
    let Reduction(clvm_cost, generator_output) =
        run_program(a, &dialect, generator_rom, args, cost_left)?;

    subtract_cost(a, &mut cost_left, clvm_cost)?;

    // we pass in what's left of max_cost here, to fail early in case the
    // cost of a condition brings us over the cost limit
    let mut result = parse_spends(a, generator_output, cost_left, flags)?;
    result.cost += max_cost - cost_left;
    Ok(result)
}

fn extract_n<const N: usize>(
    a: &Allocator,
    mut n: NodePtr,
    e: ErrorCode,
) -> Result<[NodePtr; N], ValidationErr> {
    let mut ret: [NodePtr; N] = [0; N];
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
pub fn run_block_generator2<GenBuf: AsRef<[u8]>>(
    a: &mut Allocator,
    program: &[u8],
    block_refs: &[GenBuf],
    max_cost: u64,
    flags: u32,
) -> Result<SpendBundleConditions, ValidationErr> {
    let byte_cost = program.len() as u64 * COST_PER_BYTE;

    let mut cost_left = max_cost;
    subtract_cost(a, &mut cost_left, byte_cost)?;

    let clvm_deserializer = node_from_bytes(a, &CLVM_DESERIALIZER)?;
    let program = if (flags & ALLOW_BACKREFS) != 0 {
        node_from_bytes_backrefs(a, program)?
    } else {
        node_from_bytes(a, program)?
    };

    // iterate in reverse order since we're building a linked list from
    // the tail
    let mut blocks = a.null();
    for g in block_refs.iter().rev() {
        let ref_gen = a.new_atom(g.as_ref())?;
        blocks = a.new_pair(ref_gen, blocks)?;
    }

    // the first argument to the generator is the serializer, followed by a list
    // of the blocks it requested.
    let mut args = a.new_pair(blocks, a.null())?;
    args = a.new_pair(clvm_deserializer, args)?;

    let dialect = ChiaDialect::new(flags);

    let Reduction(clvm_cost, mut all_spends) = run_program(a, &dialect, program, args, cost_left)?;

    subtract_cost(a, &mut cost_left, clvm_cost)?;
    all_spends = first(a, all_spends)?;

    // at this point all_spends is a list of:
    // (parent-coin-id puzzle-reveal amount solution . extra)
    // where extra may be nil, or additional extension data

    let mut ret = SpendBundleConditions::default();
    let mut state = ParseState::default();

    while let Some((spend, rest)) = a.next(all_spends) {
        all_spends = rest;
        // process the spend
        let [parent_id, puzzle, amount, solution, _spend_level_extra] =
            extract_n::<5>(a, spend, ErrorCode::InvalidCondition)?;

        let Reduction(clvm_cost, conditions) =
            run_program(a, &dialect, puzzle, solution, cost_left)?;

        subtract_cost(a, &mut cost_left, clvm_cost)?;

        let buf = tree_hash(a, puzzle);
        let puzzle_hash = a.new_atom(&buf)?;

        process_single_spend(
            a,
            &mut ret,
            &mut state,
            parent_id,
            puzzle_hash,
            amount,
            conditions,
            flags,
            &mut cost_left,
        )?;
    }

    validate_conditions(a, &ret, state, a.null(), flags)?;

    ret.cost = max_cost - cost_left;
    Ok(ret)
}
