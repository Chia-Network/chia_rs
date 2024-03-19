use std::{collections::HashMap, sync::Arc, time::Instant};

use chia_protocol::{BlockRecord, Bytes32, Coin, FullBlock, Program};
use clvmr::NodePtr;

use crate::{
    block_validation::{
        header_validation::{get_header_block, validate_header_block, HeaderValidationOptions},
        npc::get_npc,
    },
    consensus_constants::ConsensusConstants,
    gen::{
        conditions::SpendBundleConditions,
        validation_error::{ErrorCode, ValidationErr},
    },
};

use self::npc::NpcResult;

mod header_validation;
mod npc;
mod pot_iterations;
mod unfinished_header_validation;

pub struct PreValidationOptions {
    pub check_filter: bool,
    pub expected_difficulty: u64,
    pub expected_sub_slot_iters: u64,
    pub validate_signatures: bool,
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

pub struct BlockGenerator {
    pub program: Program,
    pub generator_refs: Vec<Program>,
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
    blocks: Arc<HashMap<Bytes32, BlockRecord>>,
    prev_generator: Option<BlockGenerator>,
    mut npc_result: Option<NpcResult>,
    constants: &ConsensusConstants,
    options: &PreValidationOptions,
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

    let header_block = get_header_block(block, additions, removals);

    let header_options = HeaderValidationOptions {
        check_filter: options.check_filter,
        expected_difficulty: options.expected_difficulty,
        expected_sub_slot_iters: options.expected_sub_slot_iters,
        check_sub_epoch_summary: true,
    };

    let result = validate_header_block(header_block, blocks, constants, &header_options);

    todo!()
}
