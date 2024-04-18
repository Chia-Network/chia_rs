use chia_protocol::Program;
use clvmr::{ENABLE_BLS_OPS_OUTSIDE_GUARD, ENABLE_FIXED_DIV, MEMPOOL_MODE};

use crate::{
    allocator::make_allocator,
    consensus_constants::ConsensusConstants,
    gen::{
        conditions::{EmptyVisitor, MempoolVisitor},
        flags::{
            AGG_SIG_ARGS, ALLOW_BACKREFS, ANALYZE_SPENDS, ENABLE_SOFTFORK_CONDITION,
            NO_RELATIVE_CONDITIONS_ON_EPHEMERAL,
        },
        owned_conditions::OwnedSpendBundleConditions,
        run_block_generator::{run_block_generator, run_block_generator2},
        validation_error::ValidationErr,
    },
};

use super::BlockGenerator;

pub type NpcResult = Result<OwnedSpendBundleConditions, ValidationErr>;

pub fn flags_for_height(height: u32, constants: &ConsensusConstants) -> u32 {
    let mut flags = 0;

    if height >= constants.soft_fork2_height {
        flags |= NO_RELATIVE_CONDITIONS_ON_EPHEMERAL;
    }

    if height >= constants.hard_fork_height {
        // The hard-fork initiated with 2.0. To activate June 2024
        // * Costs are ascribed to some unknown condition codes, to allow for
        //    soft-forking in new conditions with cost
        // * A new condition, SOFTFORK, is added which takes a first parameter to
        //   specify its cost. This allows soft-forks similar to the softfork
        //   operator
        // * BLS operators introduced in the soft-fork (behind the softfork
        //   guard) are made available outside of the guard.
        // * Division with negative numbers are allowed, and round toward
        //   negative infinity
        // * AGG_SIG_* conditions are allowed to have unknown additional
        //   arguments
        // * Allow the block generator to be serialized with the improved clvm
        //   serialization format (with back-references)
        flags = flags
            | ENABLE_SOFTFORK_CONDITION
            | ENABLE_BLS_OPS_OUTSIDE_GUARD
            | ENABLE_FIXED_DIV
            | AGG_SIG_ARGS
            | ALLOW_BACKREFS;
    }

    flags
}

pub fn get_npc(
    generator: BlockGenerator,
    max_cost: u64,
    mempool_mode: bool,
    height: u32,
    constants: &ConsensusConstants,
) -> NpcResult {
    let mut flags = flags_for_height(height, constants);

    if mempool_mode {
        flags |= MEMPOOL_MODE;
    }

    let analyze_spends = (flags & ANALYZE_SPENDS) != 0;

    let run_block = if height >= constants.hard_fork_fix_height {
        if analyze_spends {
            run_block_generator2::<Program, MempoolVisitor>
        } else {
            run_block_generator2::<Program, EmptyVisitor>
        }
    } else if analyze_spends {
        run_block_generator::<Program, MempoolVisitor>
    } else {
        run_block_generator::<Program, EmptyVisitor>
    };

    let mut allocator = make_allocator(flags);

    run_block(
        &mut allocator,
        &generator.program,
        &generator.generator_refs,
        max_cost,
        flags,
    )
    .map(|conds| OwnedSpendBundleConditions::from(&allocator, conds))
}
