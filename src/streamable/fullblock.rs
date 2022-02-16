use super::bytes::{Bytes, Bytes100, Bytes32, Bytes48, Bytes96};
use super::coin::Coin;
use super::program::ProgramArray;
use bincode::Options;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

#[pyclass(subclass, unsendable)]
#[derive(Serialize, Deserialize, Debug)]
pub struct Fullblock {
    finished_sub_slots: Vec<EndOfSubSlotBundle>,
    reward_chain_block: RewardChainBlock,
    challenge_chain_sp_proof: Option<VDFProof>, // # If not first sp in sub-slot
    challenge_chain_ip_proof: VDFProof,
    reward_chain_sp_proof: Option<VDFProof>, // # If not first sp in sub-slot
    reward_chain_ip_proof: VDFProof,
    infused_challenge_chain_ip_proof: Option<VDFProof>, // # Iff deficit < 4
    foliage: Foliage,                                   // # Reward chain foliage data
    foliage_transaction_block: Option<FoliageTransactionBlock>, // # Reward chain foliage data (tx block)
    transactions_info: Option<TransactionsInfo>, //  # Reward chain foliage data (tx block additional)
    transactions_generator: Option<ProgramArray>, //  # Program that generates transactions
    transactions_generator_ref_list: Vec<u32>, // List of block heights of previous generators referenced in this block
}

#[pymethods]
impl Fullblock {
    #[staticmethod]
    pub fn from_bytes(blob: &[u8]) -> Self {
        let chia = bincode::DefaultOptions::new()
            .with_chia_int_encoding()
            .allow_trailing_bytes()
            .with_big_endian();
        chia.deserialize(blob).unwrap()
    }

    fn __bytes__<'p>(&self, py: Python<'p>) -> &'p PyBytes {
        let chia = bincode::DefaultOptions::new()
            .with_chia_int_encoding()
            .allow_trailing_bytes()
            .with_big_endian();
        let v = chia.serialize(self).unwrap();
        PyBytes::new(py, &v)
    }

    fn generator(&self) -> Option<&[u8]> {
        self.transactions_generator.as_ref().map(|x| x.0.as_slice())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EndOfSubSlotBundle {
    challenge_chain: ChallengeChainSubSlot,
    infused_challenge_chain: Option<InfusedChallengeChainSubSlot>,
    reward_chain: RewardChainSubSlot,
    proofs: SubSlotProofs,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RewardChainBlock {
    weight: u128,
    height: u32,
    total_iters: u128,
    signage_point_index: u8,
    pos_ss_cc_challenge_hash: Bytes32,
    proof_of_space: ProofOfSpace,
    challenge_chain_sp_vdf: Option<VDFInfo>, //  # Not present for first sp in slot
    challenge_chain_sp_signature: G2Element,
    challenge_chain_ip_vdf: VDFInfo,
    reward_chain_sp_vdf: Option<VDFInfo>, // # Not present for first sp in slot
    reward_chain_sp_signature: G2Element,
    reward_chain_ip_vdf: VDFInfo,
    infused_challenge_chain_ip_vdf: Option<VDFInfo>, // # Iff deficit < 16
    is_transaction_block: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VDFProof {
    witness_type: u8,
    witness: Bytes,
    normalized_to_identity: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Foliage {
    /**
        # The entire foliage block, containing signature and the unsigned back pointer
        # The hash of this is the "header hash". Note that for unfinished blocks, the prev_block_hash
        # Is the prev from the signage point, and can be replaced with a more recent block
    */
    prev_block_hash: Bytes32,
    reward_block_hash: Bytes32,
    foliage_block_data: FoliageBlockData,
    foliage_block_data_signature: G2Element,
    foliage_transaction_block_hash: Option<Bytes32>,
    foliage_transaction_block_signature: Option<G2Element>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FoliageTransactionBlock {
    // Information that goes along with each transaction block that is relevant for light clients
    prev_transaction_block_hash: Bytes32,
    timestamp: u64,
    filter_hash: Bytes32,
    additions_root: Bytes32,
    removals_root: Bytes32,
    transactions_info_hash: Bytes32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TransactionsInfo {
    generator_root: Bytes32,      // # sha256 of the block generator in this block
    generator_refs_root: Bytes32, // # sha256 of the concatenation of the generator ref list entries
    aggregated_signature: G2Element,
    fees: u64, //  # This only includes user fees, not block rewards
    cost: u64, //  # This is the total cost of this block, including CLVM cost, cost of program size and conditions
    reward_claims_incorporated: Vec<Coin>, // # These can be in any order
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChallengeChainSubSlot {
    challenge_chain_end_of_slot_vdf: VDFInfo,
    infused_challenge_chain_sub_slot_hash: Option<Bytes32>, //  # Only at the end of a slot
    subepoch_summary_hash: Option<Bytes32>, //  # Only once per sub-epoch, and one sub-epoch delayed
    new_sub_slot_iters: Option<u64>,        //  # Only at the end of epoch, sub-epoch, and slot
    new_difficulty: Option<u64>,            //  # Only at the end of epoch, sub-epoch, and slot
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InfusedChallengeChainSubSlot {
    infused_challenge_chain_end_of_slot_vdf: VDFInfo,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RewardChainSubSlot {
    weight: u128,
    height: u32,
    total_iters: u128,
    signage_point_index: u8,
    pos_ss_cc_challenge_hash: Bytes32,
    proof_of_space: ProofOfSpace,
    challenge_chain_sp_vdf: Option<VDFInfo>, //  # Not present for first sp in slot
    challenge_chain_sp_signature: G2Element,
    challenge_chain_ip_vdf: VDFInfo,
    reward_chain_sp_vdf: Option<VDFInfo>, //  # Not present for first sp in slot
    reward_chain_sp_signature: G2Element,
    reward_chain_ip_vdf: VDFInfo,
    infused_challenge_chain_ip_vdf: Option<VDFInfo>, //  # Iff deficit < 16
    is_transaction_block: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SubSlotProofs {
    challenge_chain_slot_proof: VDFProof,
    infused_challenge_chain_slot_proof: Option<VDFProof>,
    reward_chain_slot_proof: VDFProof,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct VDFInfo {
    challenge: Bytes32, //  # Used to generate the discriminant (VDF group)
    number_of_iterations: u64,
    output: ClassgroupElement,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProofOfSpace {
    challenge: Bytes32,
    pool_public_key: Option<G1Element>, //  # Only one of these two should be present
    pool_contract_puzzle_hash: Option<Bytes32>,
    plot_public_key: G1Element,
    size: u8,
    proof: Bytes,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FoliageBlockData {
    /// # Part of the block that is signed by the plot key
    unfinished_reward_block_hash: Bytes32,
    pool_target: PoolTarget,
    pool_signature: Option<G2Element>, //  # Iff ProofOfSpace has a pool pk
    farmer_reward_puzzle_hash: Bytes32,
    extension_data: Bytes32, //  # Used for future updates. Can be any 32 byte value initially
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PoolTarget {
    puzzle_hash: Bytes32,
    max_height: u32, //  # A max height of 0 means it is valid forever
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClassgroupElement {
    data: Bytes100,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct G1Element(Bytes48);

#[derive(Serialize, Deserialize, Debug)]
pub struct G2Element(Bytes96);
