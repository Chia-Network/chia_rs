use chia_consensus::gen::conditions::{
    parse_conditions, MempoolVisitor, ParseState, Spend, SpendBundleConditions,
};
use crate::BlockGenerator;
use crate::ConsensusConstants;
use crate::gen::{run_block_generator, run_block_generator2};
use crate::get_flags_for_height_and_constants;
use crate::gen::flags::{MEMPOOL_MODE, ENABLE_MESSAGE_CONDITIONS};

#[cfg_attr(
    feature = "py-bindings",
    pyo3::pyclass(module = "chia_rs"),
    derive(PyJsonDict, PyStreamable, PyGetters),
    py_uppercase,
    py_pickle
)]
#[streamable]
pub struct NPCResult {
    error: Option<u16>,
    conds: Option<SpendBundleConditions>,
}

pub fn get_name_puzzle_conditions(
    generator: BlockGenerator,
    max_cost: i64,
    mempool_mode: bool,
    height: u32,
    constants: ConsensusConstants,
) -> NPCResult {
    let run_block = if height >= constants.HARD_FORK_FIX_HEIGHT {run_block_generator2} else {run_block_generator};
    let mut flags = get_flags_for_height_and_constants(height, constants);
    if mempool_mode {flags = flags | MEMPOOL_MODE};
    let mut block_args = Vec<&[u8]>::new();
    for gen in generator.generator_refs {
        block_args.push(gen.to_bytes());
    }
    result = run_block(generator.program.as_bytes(), block_args, max_cost, flags);
    match result {
        Err(val_err) => NPCResult(val_err, None),
        Some(val_res) => NPCResult(None, val_res),
    }
}