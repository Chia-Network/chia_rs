use std::{collections::HashMap, sync::Arc};

use chia_protocol::{
    BlockRecord, Bytes, Bytes32, ClassgroupElement, Coin, FullBlock, HeaderBlock, VDFInfo,
};
use chia_traits::Streamable;
use chiabip158::Bip158Filter;
use thiserror::Error;

use crate::{
    block_prevalidation::{
        pot_iterations::calculate_ip_iters,
        unfinished_header_validation::{
            validate_unfinished_header_block, UnfinishedHeaderValidationOptions,
        },
    },
    consensus_constants::ConsensusConstants,
    gen::validation_error::ErrorCode,
};

use super::pot_iterations::PotIterationsError;

pub fn get_block_header(
    block: FullBlock,
    additions: Vec<Coin>,
    removals: Vec<Bytes32>,
) -> HeaderBlock {
    let mut slices = Vec::new();

    if block.is_transaction_block() {
        for coin in &additions {
            slices.push(coin.puzzle_hash);
        }
        for coin in block.get_included_reward_coins() {
            slices.push(coin.puzzle_hash);
        }
        for name in &removals {
            slices.push(*name);
        }
    }

    let filter = Bip158Filter::new(&slices);
    let encoded_filter = filter.encode();

    HeaderBlock {
        finished_sub_slots: block.finished_sub_slots,
        reward_chain_block: block.reward_chain_block,
        challenge_chain_sp_proof: block.challenge_chain_sp_proof,
        challenge_chain_ip_proof: block.challenge_chain_ip_proof,
        reward_chain_sp_proof: block.reward_chain_sp_proof,
        reward_chain_ip_proof: block.reward_chain_ip_proof,
        infused_challenge_chain_ip_proof: block.infused_challenge_chain_ip_proof,
        foliage: block.foliage,
        foliage_transaction_block: block.foliage_transaction_block,
        transactions_filter: Bytes::new(encoded_filter.as_ref().to_vec()),
        transactions_info: block.transactions_info,
    }
}

pub struct HeaderValidationOptions {
    pub check_filter: bool,
    pub expected_difficulty: u64,
    pub expected_sub_slot_iters: u64,
    pub check_sub_epoch_summary: bool,
}

#[derive(Error, Debug)]
pub enum FinishedHeaderValidationError {
    #[error("error code: {0:?}")]
    ErrorCode(#[from] ErrorCode),

    #[error("pot iterations error: {0}")]
    PotIterations(#[from] PotIterationsError),
}

/// Fully validates the header of a block. A header block is the same as a full block,
/// but without transactions and transaction info. Returns `required_iters`.
pub fn validate_finished_header_block(
    header_block: HeaderBlock,
    blocks: Arc<HashMap<Bytes32, BlockRecord>>,
    constants: &ConsensusConstants,
    options: &HeaderValidationOptions,
) -> Result<u64, FinishedHeaderValidationError> {
    let unfinished_header_block = header_block.clone().into_unfinished_header_block();

    let required_iters = validate_unfinished_header_block(
        unfinished_header_block,
        blocks.clone(),
        constants,
        &UnfinishedHeaderValidationOptions {
            check_filter: options.check_filter,
            expected_difficulty: options.expected_difficulty,
            expected_sub_slot_iters: options.expected_sub_slot_iters,
            skip_overflow_last_ss_validation: false,
            skip_vdf_is_valid: false,
            check_sub_epoch_summary: options.check_sub_epoch_summary,
        },
    )?;

    let previous_block = blocks.get(&header_block.prev_header_hash());
    let new_sub_slot = !header_block.finished_sub_slots.is_empty();

    let ip_iters = calculate_ip_iters(
        constants,
        options.expected_sub_slot_iters,
        header_block.reward_chain_block.signage_point_index,
        required_iters,
    )?;

    if let Some(previous_block) = previous_block {
        // 27. Check block height.
        if header_block.height() != previous_block.height + 1 {
            return Err(ErrorCode::InvalidHeight.into());
        }

        // 28. Check weight.
        if header_block.weight() != previous_block.weight + options.expected_difficulty as u128 {
            return Err(ErrorCode::InvalidWeight.into());
        }
    } else {
        // 27b. Check genesis block height, weight, and previous block hash.

        if header_block.height() != 0 {
            return Err(ErrorCode::InvalidHeight.into());
        }

        if header_block.weight() != constants.difficulty_starting as u128 {
            return Err(ErrorCode::InvalidWeight.into());
        }

        if header_block.prev_header_hash() != constants.genesis_challenge {
            return Err(ErrorCode::InvalidPrevBlockHash.into());
        }
    }

    // RC VDF challenge is taken from more recent of (slot start, prev_block).
    let mut cc_vdf_output = ClassgroupElement::get_default_element();
    let mut ip_vdf_iters = ip_iters as u128;
    let rc_vdf_challenge: Bytes32;

    if let Some(previous_block) = previous_block {
        if new_sub_slot {
            // Slot start is more recent.
            rc_vdf_challenge = header_block
                .finished_sub_slots
                .last()
                .unwrap()
                .reward_chain
                .hash()
                .into();
        } else {
            // Previous sb is more recent.
            rc_vdf_challenge = previous_block.reward_infusion_new_challenge;
            ip_vdf_iters = header_block.reward_chain_block.total_iters - previous_block.total_iters;
            cc_vdf_output = previous_block.challenge_vdf_output;
        }
    } else if new_sub_slot {
        rc_vdf_challenge = header_block
            .finished_sub_slots
            .last()
            .unwrap()
            .reward_chain
            .hash()
            .into();
    } else {
        rc_vdf_challenge = constants.genesis_challenge;
    }

    // 29. Check challenge chain infusion point VDF.
    let cc_vdf_challenge: Bytes32;

    if new_sub_slot {
        // First block in slot.

        cc_vdf_challenge = header_block
            .finished_sub_slots
            .last()
            .unwrap()
            .challenge_chain
            .hash()
            .into();
    } else if let Some(previous_block) = previous_block {
        // Not first block in slot, and not genesis block.
        let mut current = previous_block;

        while current.finished_challenge_slot_hashes.is_none() {
            current = &blocks[&current.prev_hash];
        }

        cc_vdf_challenge = *current
            .finished_challenge_slot_hashes
            .as_ref()
            .unwrap()
            .last()
            .unwrap();
    } else {
        // Not first block in slot, but genesis block.
        cc_vdf_challenge = constants.genesis_challenge;
    }

    let cc_target_vdf_info = VDFInfo::new(
        cc_vdf_challenge,
        ip_vdf_iters as u64,
        header_block
            .reward_chain_block
            .challenge_chain_ip_vdf
            .output,
    );

    let expected_vdf_info = VDFInfo::new(
        cc_target_vdf_info.challenge,
        ip_iters,
        cc_target_vdf_info.output,
    );

    if header_block.reward_chain_block.challenge_chain_ip_vdf != expected_vdf_info {
        return Err(ErrorCode::InvalidCcIpVdf.into());
    }

    let normalized_to_identity = header_block.challenge_chain_ip_proof.normalized_to_identity;

    if !normalized_to_identity
        && !validate_vdf(
            header_block.challenge_chain_ip_proof,
            constants,
            cc_vdf_output,
            cc_target_vdf_info,
            None,
        )
    {
        return Err(ErrorCode::InvalidCcIpVdf.into());
    }

    if !normalized_to_identity
        && validate_vdf(
            header_block.challenge_chain_ip_proof,
            constants,
            ClassgroupElement::default(),
            header_block.reward_chain_block.challenge_chain_ip_vdf,
        )
    {
        return Err(ErrorCode::InvalidCcIpVdf.into());
    }

    todo!();
}
