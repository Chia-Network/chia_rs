use chia_protocol::{
    Bytes32, EndOfSubSlotBundle, Foliage, FoliageTransactionBlock, Program, RewardChainBlock,
    TransactionsInfo, VDFProof,
};
use chia_streamable_macro::streamable;
use chia_traits::Streamable;

// Proving-only copy of the pre-deferred FullBlock wire model.
// Do not export or use in production paths. This exists only so the
// equivalence checker can compare eager parsing against the public deferred
// FullBlock during the migration window.
#[streamable]
pub struct LegacyFullBlock {
    finished_sub_slots: Vec<EndOfSubSlotBundle>,
    reward_chain_block: RewardChainBlock,
    challenge_chain_sp_proof: Option<VDFProof>,
    challenge_chain_ip_proof: VDFProof,
    reward_chain_sp_proof: Option<VDFProof>,
    reward_chain_ip_proof: VDFProof,
    infused_challenge_chain_ip_proof: Option<VDFProof>,
    foliage: Foliage,
    foliage_transaction_block: Option<FoliageTransactionBlock>,
    transactions_info: Option<TransactionsInfo>,
    transactions_generator: Option<Program>,
    transactions_generator_ref_list: Vec<u32>,
}

impl LegacyFullBlock {
    pub fn prev_header_hash(&self) -> Bytes32 {
        self.foliage.prev_block_hash
    }

    pub fn header_hash(&self) -> Bytes32 {
        self.foliage.hash().into()
    }

    pub fn is_transaction_block(&self) -> bool {
        self.foliage.foliage_transaction_block_hash.is_some()
    }

    pub fn total_iters(&self) -> u128 {
        self.reward_chain_block.total_iters
    }

    pub fn height(&self) -> u32 {
        self.reward_chain_block.height
    }

    pub fn weight(&self) -> u128 {
        self.reward_chain_block.weight
    }

    pub fn is_fully_compactified(&self) -> bool {
        for sub_slot in &self.finished_sub_slots {
            if sub_slot.proofs.challenge_chain_slot_proof.witness_type != 0
                || !sub_slot
                    .proofs
                    .challenge_chain_slot_proof
                    .normalized_to_identity
            {
                return false;
            }
            if let Some(proof) = &sub_slot.proofs.infused_challenge_chain_slot_proof {
                if proof.witness_type != 0 || !proof.normalized_to_identity {
                    return false;
                }
            }
        }

        if let Some(proof) = &self.challenge_chain_sp_proof {
            if proof.witness_type != 0 || !proof.normalized_to_identity {
                return false;
            }
        }
        self.challenge_chain_ip_proof.witness_type == 0
            && self.challenge_chain_ip_proof.normalized_to_identity
    }

    pub fn transactions_generator(&self) -> Option<&Program> {
        self.transactions_generator.as_ref()
    }

    pub fn transactions_generator_ref_list(&self) -> &Vec<u32> {
        &self.transactions_generator_ref_list
    }
}
