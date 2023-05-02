use clvmr::MEMPOOL_MODE as CLVM_MEMPOOL_MODE;

// flags controlling to condition parsing

// unknown condition codes are disallowed
pub const NO_UNKNOWN_CONDS: u32 = 0x20000;

// some conditions require an exact number of arguments (AGG_SIG_UNSAFE and
// AGG_SIG_ME). This will require those argument lists to be correctly
// nil-terminated
pub const COND_ARGS_NIL: u32 = 0x40000;

// With this flag, conditions will require the exact number of arguments
// currently supported for those conditions. This is meant for mempool-mode
pub const STRICT_ARGS_COUNT: u32 = 0x80000;

// When set, support the new ASSERT_BEFORE_* conditions
pub const ENABLE_ASSERT_BEFORE: u32 = 0x100000;

// disallow relative height- and time conditions on ephemeral spends
pub const NO_RELATIVE_CONDITIONS_ON_EPHEMERAL: u32 = 0x200000;

// use the generator ROM that does not pass in the clvm parser as the first
// argument
pub const HARD_FORK: u32 = 0x400000;

pub const MEMPOOL_MODE: u32 = CLVM_MEMPOOL_MODE
    | NO_UNKNOWN_CONDS
    | COND_ARGS_NIL
    | STRICT_ARGS_COUNT
    | NO_RELATIVE_CONDITIONS_ON_EPHEMERAL;
