use chia_streamable_macro::Streamable;

use crate::streamable_struct;
use crate::EndOfSubSlotBundle;
use crate::Program;
use crate::RewardChainBlock;
use crate::VDFProof;
use crate::{Foliage, FoliageTransactionBlock, TransactionsInfo};

streamable_struct! (FullBlock {
    finished_sub_slots: Vec<EndOfSubSlotBundle>,
    reward_chain_block: RewardChainBlock,
    challenge_chain_sp_proof: Option<VDFProof>, // # If not first sp in sub-slot
    challenge_chain_ip_proof: VDFProof,
    reward_chain_sp_proof: Option<VDFProof>, // # If not first sp in sub-slot
    reward_chain_ip_proof: VDFProof,
    infused_challenge_chain_ip_proof: Option<VDFProof>, // # Iff deficit < 4
    foliage: Foliage,                                   // # Reward chain foliage data
    foliage_transaction_block: Option<FoliageTransactionBlock>, // # Reward chain foliage data (tx block)
    transactions_info: Option<TransactionsInfo>, // Reward chain foliage data (tx block additional)
    transactions_generator: Option<Program>,     // Program that generates transactions
    transactions_generator_ref_list: Vec<u32>, // List of block heights of previous generators referenced in this block
});
