use std::{collections::HashMap, sync::Arc};

use chia_protocol::{BlockRecord, Bytes, Bytes32, Coin, FullBlock, HeaderBlock};
use chiabip158::Bip158Filter;

use crate::{
    block_prevalidation::unfinished_header_validation::{
        validate_unfinished_header_block, UnfinishedHeaderValidationOptions,
    },
    consensus_constants::ConsensusConstants,
    gen::validation_error::ErrorCode,
};

pub fn get_header_block(
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

/// Returns required iters.
pub fn validate_finished_header_block(
    header_block: HeaderBlock,
    blocks: Arc<HashMap<Bytes32, BlockRecord>>,
    constants: &ConsensusConstants,
    options: &HeaderValidationOptions,
) -> Result<u64, ErrorCode> {
    let unfinished_header_block = header_block.into_unfinished_header_block();

    let unfinished_options = UnfinishedHeaderValidationOptions {
        check_filter: options.check_filter,
        expected_difficulty: options.expected_difficulty,
        expected_sub_slot_iters: options.expected_sub_slot_iters,
        skip_overflow_last_ss_validation: false,
        skip_vdf_is_valid: false,
        check_sub_epoch_summary: options.check_sub_epoch_summary,
    };

    let required_iters = validate_unfinished_header_block(
        unfinished_header_block,
        blocks,
        constants,
        &unfinished_options,
    )?;

    todo!();
}
