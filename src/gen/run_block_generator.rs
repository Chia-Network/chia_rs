use crate::gen::conditions::{parse_spends, SpendBundleConditions};
use crate::gen::validation_error::ValidationErr;
use crate::generator_rom::{COST_PER_BYTE, GENERATOR_ROM};
use clvmr::allocator::Allocator;
use clvmr::chia_dialect::ChiaDialect;
use clvmr::reduction::Reduction;
use clvmr::run_program::run_program;
use clvmr::serde::node_from_bytes;

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
    let byte_cost = program.len() as u64 * COST_PER_BYTE;

    let generator_rom = node_from_bytes(a, &GENERATOR_ROM)?;
    let program = node_from_bytes(a, program)?;

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
        run_program(a, &dialect, generator_rom, args, max_cost - byte_cost)?;

    // we pass in what's left of max_cost here, to fail early in case the
    // cost of a condition brings us over the cost limit
    let mut result = parse_spends(a, generator_output, max_cost - clvm_cost - byte_cost, flags)?;
    result.cost += clvm_cost + byte_cost;
    Ok(result)
}
