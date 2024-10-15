use clvmr::MEMPOOL_MODE as CLVM_MEMPOOL_MODE;

// flags controlling the condition parsing
// These flags are combined in the same fields as clvm_rs flags, controlling the
// CLVM execution. To avoid clashes, CLVM flags are in the lower two bytes and
// condition parsing and validation flags are in the top two bytes.

// unknown condition codes are disallowed
pub const NO_UNKNOWN_CONDS: u32 = 0x2_0000;

// With this flag, conditions will require the exact number of arguments
// currently supported for those conditions. This is meant for mempool-mode
pub const STRICT_ARGS_COUNT: u32 = 0x8_0000;

// when this flag is set, the block generator serialization is allowed to
// contain back-references
pub const ALLOW_BACKREFS: u32 = 0x0200_0000;

// By default, run_block_generator validates the signatures of any AGG_SIG
// condition. By passing in this flag, the signatures are not validated (saving
// time). This is useful when we've already validated a block but we need to
// re-run it to compute additions and removals.
pub const DONT_VALIDATE_SIGNATURE: u32 = 0x1_0000;

pub const MEMPOOL_MODE: u32 = CLVM_MEMPOOL_MODE | NO_UNKNOWN_CONDS | STRICT_ARGS_COUNT;
