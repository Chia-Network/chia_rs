use std::time::Instant;

use chia_protocol::{Bytes32, Coin, FullBlock, Program};
use clvmr::{NodePtr, ENABLE_BLS_OPS_OUTSIDE_GUARD, ENABLE_FIXED_DIV, MEMPOOL_MODE};

use crate::{
    allocator::make_allocator,
    consensus_constants::ConsensusConstants,
    gen::{
        conditions::{EmptyVisitor, MempoolVisitor, SpendBundleConditions},
        flags::{
            AGG_SIG_ARGS, ALLOW_BACKREFS, ANALYZE_SPENDS, ENABLE_SOFTFORK_CONDITION,
            NO_RELATIVE_CONDITIONS_ON_EPHEMERAL,
        },
        run_block_generator::{run_block_generator, run_block_generator2},
        validation_error::{ErrorCode, ValidationErr},
    },
};

pub type NpcResult = Result<SpendBundleConditions, ValidationErr>;

pub struct BlockValidationOptions {
    pub check_filter: bool,
    pub expected_difficulty: u64,
    pub expected_sub_slot_iters: u64,
    pub validate_signatures: bool,
}

pub struct BlockGenerator {
    pub program: Program,
    pub generator_refs: Vec<Program>,
}

pub struct PreValidationResult {
    /// An error, if applicable.
    pub error: Option<ValidationErr>,

    /// If `error` is `None`.
    pub required_iters: Option<u64>,

    /// If `error` is `None` and the block is a transaction block.
    pub npc_result: Option<NpcResult>,

    /// Whether or not signatures were validated.
    pub validated_signature: bool,

    /// The time (in milliseconds) it took to pre-validate the block.
    pub timing: u32,
}

macro_rules! validate {
    ($condition:expr, $start_time:ident) => {
        if !$condition {
            validation_err!($start_time);
        }
    };
}

macro_rules! validation_err {
    ($start_time:ident) => {
        return PreValidationResult {
            error: Some(ValidationErr(NodePtr::NIL, ErrorCode::Unknown)),
            required_iters: None,
            npc_result: None,
            validated_signature: false,
            timing: $start_time.elapsed().as_millis() as u32,
        };
    };
}

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
}

pub fn removals_and_additions(conditions: &SpendBundleConditions) -> (Vec<Bytes32>, Vec<Coin>) {
    let mut removals = Vec::new();
    let mut additions = Vec::new();

    for spend in conditions.spends.iter() {
        removals.push(*spend.coin_id);
        for new_coin in spend.create_coin.iter() {
            additions.push(Coin {
                parent_coin_info: *spend.coin_id,
                puzzle_hash: new_coin.puzzle_hash,
                amount: new_coin.amount,
            });
        }
    }

    (removals, additions)
}

pub fn pre_validate_block(
    block: FullBlock,
    prev_generator: Option<BlockGenerator>,
    mut npc_result: Option<NpcResult>,
    constants: &ConsensusConstants,
    options: &BlockValidationOptions,
) -> PreValidationResult {
    let start_time = Instant::now();

    let (removals, additions) = if let Some(Ok(conditions)) = &npc_result {
        removals_and_additions(conditions)
    } else if let Some(generator) = block.transactions_generator.as_ref().and_then(|generator| {
        if npc_result.is_none() {
            None
        } else {
            Some(generator)
        }
    }) {
        let Some(prev_generator) = prev_generator else {
            validation_err!(start_time);
        };

        validate!(&prev_generator.program == generator, start_time);

        let max_cost = constants.max_block_cost_clvm.min(
            block
                .transactions_info
                .as_ref()
                .map(|info| info.cost)
                .unwrap_or(0),
        );

        let npc = get_npc(prev_generator, max_cost, false, block.height(), constants);
        let (removals, additions) = npc.as_ref().map(removals_and_additions).unwrap_or_default();
        npc_result = Some(npc);

        (removals, additions)
    } else if let Some(Err(error)) = &npc_result {
        return PreValidationResult {
            error: Some(*error),
            required_iters: None,
            npc_result,
            validated_signature: false,
            timing: start_time.elapsed().as_millis() as u32,
        };
    } else {
        Default::default()
    };

    todo!()
}
