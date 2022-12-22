use chia_streamable_macro::Streamable;

use crate::chia_error;
use crate::streamable_struct;
use crate::Bytes32;
use crate::Coin;
use crate::G2Element;
use crate::PoolTarget;
use crate::Streamable;

#[cfg(feature = "py-bindings")]
use crate::from_json_dict::FromJsonDict;
#[cfg(feature = "py-bindings")]
use crate::to_json_dict::ToJsonDict;
#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::PyStreamable;
#[cfg(feature = "py-bindings")]
use pyo3::prelude::*;

streamable_struct! (TransactionsInfo {
    // Information that goes along with each transaction block
    generator_root: Bytes32, // sha256 of the block generator in this block
    generator_refs_root: Bytes32, // sha256 of the concatenation of the generator ref list entries
    aggregated_signature: G2Element,
    fees: u64, // This only includes user fees, not block rewards
    cost: u64, // This is the total cost of this block, including CLVM cost, cost of program size and conditions
    reward_claims_incorporated: Vec<Coin>, // These can be in any order
});

streamable_struct!(FoliageTransactionBlock {
    // Information that goes along with each transaction block that is relevant for light clients
    prev_transaction_block_hash: Bytes32,
    timestamp: u64,
    filter_hash: Bytes32,
    additions_root: Bytes32,
    removals_root: Bytes32,
    transactions_info_hash: Bytes32,
});

streamable_struct! (FoliageBlockData {
    // Part of the block that is signed by the plot key
    unfinished_reward_block_hash: Bytes32,
    pool_target: PoolTarget,
    pool_signature: Option<G2Element>, // Iff ProofOfSpace has a pool pk
    farmer_reward_puzzle_hash: Bytes32,
    extension_data: Bytes32, // Used for future updates. Can be any 32 byte value initially
});

streamable_struct! (Foliage {
    // The entire foliage block, containing signature and the unsigned back pointer
    // The hash of this is the "header hash". Note that for unfinished blocks, the prev_block_hash
    // Is the prev from the signage point, and can be replaced with a more recent block
    prev_block_hash: Bytes32,
    reward_block_hash: Bytes32,
    foliage_block_data: FoliageBlockData,
    foliage_block_data_signature: G2Element,
    foliage_transaction_block_hash: Option<Bytes32>,
    foliage_transaction_block_signature: Option<G2Element>,
});
