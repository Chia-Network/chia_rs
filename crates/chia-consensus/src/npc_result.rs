use crate::allocator::make_allocator;
use crate::consensus_constants::ConsensusConstants;
use crate::gen::conditions::EmptyVisitor;
use crate::gen::conditions::SpendBundleConditions;
use crate::gen::flags::MEMPOOL_MODE;
use crate::gen::owned_conditions::OwnedSpendBundleConditions;
use crate::gen::run_block_generator::{run_block_generator, run_block_generator2};
use crate::gen::validation_error::{ErrorCode, ValidationErr};
use crate::multiprocess_validation::get_flags_for_height_and_constants;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::{PyGetters, PyJsonDict, PyStreamable};
use clvmr::chia_dialect::LIMIT_HEAP;

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

pub fn get_name_puzzle_conditions(
    generator_program: Vec<u8>,
    max_cost: u64,
    mempool_mode: bool,
    height: u32,
    constants: &ConsensusConstants,
) -> Result<OwnedSpendBundleConditions, ErrorCode> {
    let mut flags = get_flags_for_height_and_constants(height, constants);
    if mempool_mode {
        flags |= MEMPOOL_MODE
    };
    let block_args = Vec::<Vec<u8>>::new();
    let mut a2 = make_allocator(LIMIT_HEAP);
    let sbc_result: Result<SpendBundleConditions, ValidationErr> =
        if height >= constants.hard_fork_fix_height {
            run_block_generator2::<_, EmptyVisitor>(
                &mut a2,
                generator_program.as_slice(),
                &block_args,
                max_cost,
                flags,
            )
        } else {
            run_block_generator::<_, EmptyVisitor>(
                &mut a2,
                generator_program.as_slice(),
                &block_args,
                max_cost,
                flags,
            )
        };
    match sbc_result {
        Ok(sbc) => {
            let result = OwnedSpendBundleConditions::from(&a2, sbc);
            match result {
                Ok(r) => Ok(r),
                Err(_) => Err(ErrorCode::InvalidSpendBundle),
            }
        }
        Err(e) => Err(e.1),
    }
}
