use chia_protocol::Bytes32;
use chia_streamable_macro::streamable;

#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::{PyGetters, PyJsonDict, PyStreamable};

#[cfg_attr(
    feature = "py-bindings",
    pyo3::pyclass(module = "chia_rs"),
    derive(PyJsonDict, PyStreamable, PyGetters),
    py_uppercase,
    py_pickle
)]
#[streamable]
pub struct ConsensusConstants {
    /// How many blocks to target per sub-slot.
    slot_blocks_target: u32,

    /// How many blocks must be created per slot (to make challenge sb).
    min_blocks_per_challenge_block: u8,

    /// Max number of blocks that can be infused into a sub-slot.
    /// Note: This must be less than SUB_EPOCH_BLOCKS/2, and > SLOT_BLOCKS_TARGET.
    max_sub_slot_blocks: u32,

    /// The number of signage points per sub-slot (including the 0th sp at the sub-slot start).
    num_sps_sub_slot: u32,

    /// The sub_slot_iters for the first epoch.
    sub_slot_iters_starting: u64,

    /// Multiplied by the difficulty to get iterations.
    difficulty_constant_factor: u128,

    /// The difficulty for the first epoch.
    difficulty_starting: u64,

    /// The maximum factor by which difficulty and sub_slot_iters can change per epoch.
    difficulty_change_max_factor: u32,

    /// The number of blocks per sub-epoch.
    sub_epoch_blocks: u32,

    /// The number of blocks per sub-epoch, must be a multiple of SUB_EPOCH_BLOCKS.
    epoch_blocks: u32,

    /// The number of bits to look at in difficulty and min iters. The rest are zeroed.
    significant_bits: u8,

    /// Max is 1024 (based on ClassGroupElement int size).
    discriminant_size_bits: u16,

    /// H(plot id + challenge hash + signage point) must start with these many zeroes.
    number_zero_bits_plot_filter: u8,

    min_plot_size: u8,

    max_plot_size: u8,

    /// The target number of seconds per sub-slot.
    sub_slot_time_target: u16,

    /// The difference between signage point and infusion point (plus required_iters).
    num_sp_intervals_extra: u8,

    /// After soft-fork2, this is the new MAX_FUTURE_TIME.
    max_future_time2: u32,

    /// Than the average of the last NUMBER_OF_TIMESTAMPS blocks.
    number_of_timestamps: u8,

    /// Used as the initial cc rc challenges, as well as first block back pointers, and first SES back pointer.
    /// We override this value based on the chain being run (testnet0, testnet1, mainnet, etc).
    genesis_challenge: Bytes32,

    /// Forks of chia should change this value to provide replay attack protection.
    agg_sig_me_additional_data: Bytes32,

    /// The block at height must pay out to this pool puzzle hash.
    genesis_pre_farm_pool_puzzle_hash: Bytes32,

    /// The block at height must pay out to this farmer puzzle hash.
    genesis_pre_farm_farmer_puzzle_hash: Bytes32,

    /// The maximum number of classgroup elements within an n-wesolowski proof.
    max_vdf_witness_size: u8,

    /// Size of mempool = 10x the size of block.
    mempool_block_buffer: u8,

    /// Max coin amount uint(1 << 64). This allows coin amounts to fit in 64 bits. This is around 18M chia.
    max_coin_amount: u64,

    /// Max block cost in clvm cost units.
    max_block_cost_clvm: u64,

    /// Cost per byte of generator program.
    cost_per_byte: u64,

    weight_proof_threshold: u8,

    weight_proof_recent_blocks: u32,

    max_block_count_per_requests: u32,

    blocks_cache_size: u32,

    max_generator_size: u32,

    max_generator_ref_list_size: u32,

    pool_sub_slot_iters: u64,

    /// Soft fork initiated in 1.8.0 release.
    soft_fork2_height: u32,

    /// Soft fork initiated in 2.3.0 release.
    soft_fork4_height: u32,

    /// The hard fork planned with the 2.0 release.
    /// This is the block with the first plot filter adjustment.
    hard_fork_height: u32,

    hard_fork_fix_height: u32,

    /// The 128 plot filter adjustment height.
    plot_filter_128_height: u32,

    /// The 64 plot filter adjustment height.
    plot_filter_64_height: u32,

    /// The 32 plot filter adjustment height.
    plot_filter_32_height: u32,
}
