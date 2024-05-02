use crate::gen::conditions::{
    parse_conditions, MempoolVisitor, ParseState, Spend, SpendBundleConditions,
};
use crate::gen::owned_conditions::OwnedSpendBundleConditions;
use crate::gen::validation_error::ValidationErr;
use crate::generator_types::BlockGenerator;
use crate::consensus_constants::ConsensusConstants;
use crate::gen::run_block_generator::{run_block_generator, run_block_generator2};
use crate::multiprocess_validation::get_flags_for_height_and_constants;
use crate::gen::flags::MEMPOOL_MODE;
use chia_streamable_macro::streamable;
use chia_protocol::Program;
use crate::allocator::make_allocator;
use clvmr::chia_dialect::LIMIT_HEAP;

#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::{PyGetters, PyJsonDict, PyStreamable};

// we may be able to remove this struct and just return a Rust native Result

// #[cfg_attr(
//     feature = "py-bindings",
//     pyo3::pyclass(module = "chia_rs"),
//     derive(PyJsonDict, PyStreamable, PyGetters),
//     py_uppercase,
//     py_pickle
// )]
// #[streamable]
// pub struct NPCResult {
//     error: Option<u16>,
//     conds: Option<OwnedSpendBundleConditions>,
// }

pub fn get_name_puzzle_conditions<GenBuf: AsRef<[u8]>(
    generator: BlockGenerator,
    max_cost: u64,
    mempool_mode: bool,
    height: u32,
    constants: ConsensusConstants,
) -> Result<SpendBundleConditions, ValidationErr> {
    let run_block: fn(&mut Allocator,
        &[u8],
        &[GenBuf],
        u64,
        u32
    ) = 
        if height >= constants.hard_fork_fix_height {run_block_generator2} 
        else {run_block_generator};
    let mut flags = get_flags_for_height_and_constants(height, constants);
    if mempool_mode {flags = flags | MEMPOOL_MODE};
    let mut block_args = Vec::<&[u8]>::new();
    for gen in generator.generator_refs {
        block_args.push(gen.into_inner().as_slice());
    }
    let mut a2 = make_allocator(LIMIT_HEAP);
    run_block(&mut a2, generator.program.into_inner().as_slice(), &block_args, max_cost, flags)
}