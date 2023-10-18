use chia_streamable_macro::Streamable;

use crate::streamable_struct;
use crate::EndOfSubSlotBundle;
use crate::Program;
use crate::RewardChainBlockUnfinished;
use crate::VDFProof;
use crate::{Foliage, FoliageTransactionBlock, TransactionsInfo};

streamable_struct! (UnfinishedBlock {
    // Full block, without the final VDFs
    finished_sub_slots: Vec<EndOfSubSlotBundle>,  // If first sb
    reward_chain_block: RewardChainBlockUnfinished,  // Reward chain trunk data
    challenge_chain_sp_proof: Option<VDFProof>,  // If not first sp in sub-slot
    reward_chain_sp_proof: Option<VDFProof>,  // If not first sp in sub-slot
    foliage: Foliage,  // Reward chain foliage data
    foliage_transaction_block: Option<FoliageTransactionBlock>,  // Reward chain foliage data (tx block)
    transactions_info: Option<TransactionsInfo>,  // Reward chain foliage data (tx block additional)
    transactions_generator: Option<Program>,  // Program that generates transactions
    transactions_generator_ref_list: Vec<u32>,  // List of block heights of previous generators referenced in this block
});
