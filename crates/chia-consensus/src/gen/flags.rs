use clvmr::MEMPOOL_MODE as CLVM_MEMPOOL_MODE;

// flags controlling to condition parsing

// unknown condition codes are disallowed
pub const NO_UNKNOWN_CONDS: u32 = 0x20000;

// With this flag, conditions will require the exact number of arguments
// currently supported for those conditions. This is meant for mempool-mode
pub const STRICT_ARGS_COUNT: u32 = 0x80000;

// when this flag is set, the block generator serialization is allowed to
// contain back-references
pub const ALLOW_BACKREFS: u32 = 0x0200_0000;

// When set, the "flags" field of the Spend objects will be set depending on
// what features are detected of the spends
pub const ANALYZE_SPENDS: u32 = 0x0400_0000;

// When this flag is set, we reject AGG_SIG_* conditions whose public key is the
// infinity G1 point. Such public keys are mathematically valid, but do not
// provide any security guarantees. Chia has historically allowed them. Enabling
// this flag is a soft-fork.
pub const DISALLOW_INFINITY_G1: u32 = 0x1000_0000;

pub const MEMPOOL_MODE: u32 = CLVM_MEMPOOL_MODE
    | NO_UNKNOWN_CONDS
    | STRICT_ARGS_COUNT
    | ANALYZE_SPENDS
    | DISALLOW_INFINITY_G1;
