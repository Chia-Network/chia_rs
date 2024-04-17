use std::{collections::HashMap, sync::Arc, time::Instant};

use chia_bls::{aggregate_verify, PublicKey};
use chia_protocol::{BlockRecord, Bytes32, Coin, FullBlock, Program};
use clvmr::NodePtr;

use crate::{
    block_prevalidation::{
        finished_header_validation::{
            get_block_header, validate_finished_header_block, HeaderValidationOptions,
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

fn removals_and_additions(conditions: &OwnedSpendBundleConditions) -> (Vec<Bytes32>, Vec<Coin>) {
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
    cached_npc_result: Option<NpcResult>,
    constants: &ConsensusConstants,
    options: &PreValidationOptions,
) -> PreValidationResult {
    let start_time = Instant::now();
    let npc_result;

    let mut removals = Vec::new();
    let mut additions = Vec::new();

    let transactions_info = block.transactions_info.clone();

    // Use the cached NPC result if present.
    if let Some(cached_npc_result) = cached_npc_result {
        match cached_npc_result {
            Ok(conditions) => {
                (removals, additions) = removals_and_additions(&conditions);
                npc_result = Some(NpcResult::Ok(conditions));
            }
            Err(error) => {
                return PreValidationResult::error(error, Some(cached_npc_result), start_time)
            }
        }
    } else if let Some(transactions_generator) = block.transactions_generator.clone() {
        let Some(prev_generator) = prev_generator else {
            return PreValidationResult::unknown(start_time);
        };

        let Some(transactions_info) = transactions_info.as_ref() else {
            return PreValidationResult::unknown(start_time);
        };

        if prev_generator.program != transactions_generator {
            return PreValidationResult::unknown(start_time);
        }

        let Ok(conditions) = get_npc(
            prev_generator,
            constants.max_block_cost_clvm.min(transactions_info.cost),
            false,
            block.height(),
            constants,
        ) else {
            return PreValidationResult::unknown(start_time);
        };

        (removals, additions) = removals_and_additions(&conditions);
        npc_result = Some(NpcResult::Ok(conditions));
    } else {
        npc_result = None;
    }

    let header_block = get_block_header(block, additions, removals);

    let mut result = validate_finished_header_block(
        header_block,
        blocks,
        constants,
        &HeaderValidationOptions {
            check_filter: options.check_filter,
            expected_difficulty: options.expected_difficulty,
            expected_sub_slot_iters: options.expected_sub_slot_iters,
            check_sub_epoch_summary: true,
        },
    );

    if let Err(error_code) = result {
        return PreValidationResult::error(error_code.into(), npc_result, start_time);
    }

    let mut signatures_valid = false;

    if options.validate_signatures {
        if let (Some(npc_result), Some(transactions_info)) = (&npc_result, transactions_info) {
            let Ok(conditions) = npc_result else {
                return PreValidationResult::unknown(start_time);
            };

            let Some(pairs) = pkm_pairs(conditions, constants.agg_sig_me_additional_data) else {
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
                result = Err(ErrorCode::BadAggregateSignature);
            }

            signatures_valid = true;
        }
    };

    PreValidationResult {
        error: result.err().map(Into::into),
        required_iters: result.ok(),
        npc_result,
        validated_signature: signatures_valid,
        timing: start_time.elapsed().as_millis() as u32,
    }
}
