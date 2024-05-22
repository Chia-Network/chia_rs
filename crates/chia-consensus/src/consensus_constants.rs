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


pub const TEST_CONSTANTS: ConsensusConstants = ConsensusConstants {
    slot_blocks_target: 32,
    min_blocks_per_challenge_block: 16,
    max_sub_slot_blocks: 128, 
    num_sps_sub_slot: 64,
    sub_slot_iters_starting: u64::pow(2,27),
    
    difficulty_constant_factor: u128::pow(2,67),
    difficulty_starting: 7,
    difficulty_change_max_factor: 3, 

    sub_epoch_blocks: 384,  
    epoch_blocks: 4608,
    significant_bits: 8,
    discriminant_size_bits: 1024,
    number_zero_bits_plot_filter: 9,
    min_plot_size: 32,
    max_plot_size: 50,
    sub_slot_time_target: 600,
    num_sp_intervals_extra: 3, 
    max_future_time2: 2 * 60, 
    number_of_timestamps: 11,  
    genesis_challenge: Bytes32::const_new([
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14,
        0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9, 0x24,
        0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c,
        0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55,
    ]),

    agg_sig_me_additional_data: Bytes32::const_new([
        0xcc, 0xd5, 0xbb, 0x71, 0x18, 0x35, 0x32, 0xbf,
        0xf2, 0x20, 0xba, 0x46, 0xc2, 0x68, 0x99, 0x1a,
        0x3f, 0xf0, 0x7e, 0xb3, 0x58, 0xe8, 0x25, 0x5a,
        0x65, 0xc3, 0x0a, 0x2d, 0xce, 0x0e, 0x5f, 0xbb,
    ]),
    genesis_pre_farm_pool_puzzle_hash: Bytes32::const_new([
        0xd2, 0x3d, 0xa1, 0x46, 0x95, 0xa1, 0x88, 0xae,
        0x57, 0x08, 0xdd, 0x15, 0x22, 0x63, 0xc4, 0xdb,
        0x88, 0x3e, 0xb2, 0x7e, 0xde, 0xb9, 0x36, 0x17,
        0x8d, 0x4d, 0x98, 0x8b, 0x8f, 0x3c, 0xe5, 0xfc,
    ]),
    genesis_pre_farm_farmer_puzzle_hash: Bytes32::const_new([
        0x3d, 0x87, 0x65, 0xd3, 0xa5, 0x97, 0xec, 0x1d,
        0x99, 0x66, 0x3f, 0x6c, 0x98, 0x16, 0xd9, 0x15,
        0xb9, 0xf6, 0x86, 0x13, 0xac, 0x94, 0x00, 0x98,
        0x84, 0xc4, 0xad, 0xda, 0xef, 0xcc, 0xe6, 0xaf,
    ]),
    max_vdf_witness_size: 64,

    mempool_block_buffer: 10,

    max_coin_amount: ((1u128 << 64) - 1) as u64,

    max_block_cost_clvm: 11000000000,

    cost_per_byte: 12000,
    weight_proof_threshold: 2,
    blocks_cache_size: 4608 + (128 * 4),
    weight_proof_recent_blocks: 1000,
    max_block_count_per_requests: 32,
    max_generator_size: 1000000,
    max_generator_ref_list_size: 512,
    pool_sub_slot_iters: 37600000000,
    soft_fork2_height: 0,
    soft_fork4_height: 5716000,
    hard_fork_height: 5496000,
    hard_fork_fix_height: 5496000,
    plot_filter_128_height: 10542000,
    plot_filter_64_height: 15592000,
    plot_filter_32_height: 20643000,
};