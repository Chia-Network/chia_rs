use std::{collections::HashMap, sync::Arc, time::Instant};

use chia_bls::{aggregate_verify, PublicKey};
use chia_protocol::{BlockRecord, Bytes32, Coin, FullBlock, Program};
use clvmr::NodePtr;

use crate::{
    block_prevalidation::{
        finished_header_validation::{
            get_header_block, validate_finished_header_block, HeaderValidationOptions,
        },
        npc::get_npc,
        signature_validation::pkm_pairs,
    },
    consensus_constants::ConsensusConstants,
    gen::{
        owned_conditions::OwnedSpendBundleConditions,
        validation_error::{ErrorCode, ValidationErr},
    },
};

use self::npc::NpcResult;

mod finished_header_validation;
mod npc;
mod pot_iterations;
mod signature_validation;
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

impl PreValidationResult {
    pub fn error(error: ValidationErr, npc_result: Option<NpcResult>, start_time: Instant) -> Self {
        Self {
            error: Some(error),
            required_iters: None,
            npc_result,
            validated_signature: false,
            timing: start_time.elapsed().as_millis() as u32,
        }
    }

    pub fn unknown(start_time: Instant) -> Self {
        Self::error(
            ValidationErr(NodePtr::NIL, ErrorCode::Unknown),
            None,
            start_time,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockGenerator {
    pub program: Program,
    pub generator_refs: Vec<Program>,
}

pub fn removals_and_additions(
    conditions: &OwnedSpendBundleConditions,
) -> (Vec<Bytes32>, Vec<Coin>) {
    let mut removals = Vec::new();
    let mut additions = Vec::new();

    for spend in conditions.spends.iter() {
        removals.push(spend.coin_id);
        for (puzzle_hash, amount, _) in spend.create_coin.iter() {
            additions.push(Coin {
                parent_coin_info: spend.coin_id,
                puzzle_hash: *puzzle_hash,
                amount: *amount,
            });
        }
    }

    (removals, additions)
}

pub fn pre_validate_block(
    block: FullBlock,
    blocks: Arc<HashMap<Bytes32, BlockRecord>>,
    prev_generator: Option<BlockGenerator>,
    npc_result: Option<NpcResult>,
    constants: &ConsensusConstants,
    options: &PreValidationOptions,
) -> PreValidationResult {
    let start_time = Instant::now();

    let (removals, additions) = match (&npc_result, block.transactions_generator.clone()) {
        // If we have a valid NPC result, we can use that to get the removals and additions.
        (Some(Ok(conditions)), _) => removals_and_additions(conditions),

        // The NPC result is an error.
        (Some(Err(error)), _) => return PreValidationResult::error(*error, npc_result, start_time),

        // If we have a generator, we can use that to get the removals and additions.
        (None, Some(transactions_generator)) => {
            let prev_generator = prev_generator.expect("missing previous generator");

            if transactions_generator != prev_generator.program {
                return PreValidationResult::unknown(start_time);
            }

            let block_cost = block
                .transactions_info
                .as_ref()
                .expect("missing transactions info")
                .cost;

            let Ok(conditions) = get_npc(
                prev_generator,
                constants.max_block_cost_clvm.min(block_cost),
                false,
                block.height(),
                constants,
            ) else {
                return PreValidationResult::unknown(start_time);
            };

            removals_and_additions(&conditions)
        }

        // We don't have either a valid NPC result or a generator, so we can't get the removals and additions.
        (None, None) => Default::default(),
    };

    let header_block = get_header_block(block.clone(), additions, removals);

    let header_options = HeaderValidationOptions {
        check_filter: options.check_filter,
        expected_difficulty: options.expected_difficulty,
        expected_sub_slot_iters: options.expected_sub_slot_iters,
        check_sub_epoch_summary: true,
    };

    let (required_iters, mut error_code) =
        match validate_finished_header_block(header_block, blocks, constants, &header_options) {
            Ok(required_iters) => (Some(required_iters), None),
            Err(error_code) => (None, Some(error_code)),
        };

    let should_validate_signatures = error_code.is_none() && options.validate_signatures;
    let signatures_valid = if should_validate_signatures {
        match (&npc_result, block.transactions_info) {
            (Some(npc_result), Some(transactions_info)) => {
                let Ok(conditions) = npc_result else {
                    return PreValidationResult::unknown(start_time);
                };

                let Some(pairs) = pkm_pairs(conditions, constants.agg_sig_me_additional_data)
                else {
                    return PreValidationResult::unknown(start_time);
                };

                let mut data = Vec::with_capacity(pairs.len());

                for (pk, msg) in pairs {
                    let Ok(pk) = PublicKey::from_bytes_unchecked(&pk.to_bytes()) else {
                        return PreValidationResult::unknown(start_time);
                    };
                    data.push((pk, msg.into_inner()));
                }

                if !aggregate_verify(&transactions_info.aggregated_signature, data) {
                    error_code = Some(ErrorCode::BadAggregateSignature);
                }

                true
            }
            _ => false,
        }
    } else {
        false
    };

    PreValidationResult {
        error: error_code.map(|code| ValidationErr(NodePtr::NIL, code)),
        required_iters,
        npc_result,
        validated_signature: signatures_valid,
        timing: start_time.elapsed().as_millis() as u32,
    }
}
