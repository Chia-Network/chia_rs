use chia_streamable_macro::Streamable;

use crate::streamable_struct;
use crate::Bytes;
use crate::EndOfSubSlotBundle;
use crate::RewardChainBlock;
use crate::VDFProof;
use crate::{Foliage, FoliageTransactionBlock, TransactionsInfo};

streamable_struct! (HeaderBlock {
    // If first sb
    finished_sub_slots: Vec<EndOfSubSlotBundle>,
    // Reward chain trunk data
    reward_chain_block: RewardChainBlock,
    // If not first sp in sub-slot
    challenge_chain_sp_proof: Option<VDFProof>,
    challenge_chain_ip_proof: VDFProof,
    // If not first sp in sub-slot
    reward_chain_sp_proof: Option<VDFProof>,
    reward_chain_ip_proof: VDFProof,
    // Iff deficit < 4
    infused_challenge_chain_ip_proof: Option<VDFProof>,
    // Reward chain foliage data
    foliage: Foliage,
    // Reward chain foliage data (tx block)
    foliage_transaction_block: Option<FoliageTransactionBlock>,
    // Filter for block transactions
    transactions_filter: Bytes,
    // Reward chain foliage data (tx block additional)
    transactions_info: Option<TransactionsInfo>,
});
