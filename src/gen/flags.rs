use clvmr::MEMPOOL_MODE as CLVM_MEMPOOL_MODE;

// flags controlling to condition parsing

// When set, limit the number of atoms allowed to be allocated to 62'500'000
// Also limit the number of pairs allowed to be allocated to the same limit
// This is similar to LIMIT_HEAP, except this flag only limits the total number
// of pairs and atoms we allow to be allocated, not the total heap size
pub const LIMIT_OBJECTS: u32 = 0x10000;

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

// enable softfork condition. Enabling this flag is a hard fork
pub const ENABLE_SOFTFORK_CONDITION: u32 = 0x400000;

// this lifts the restriction that AGG_SIG_ME and AGG_SIG_UNSAFE are only
// allowed to have two arguments. This makes the AGG_SIG_* conditions behave
// normal, just like all other conditions. Setting this flag is a hard fork
pub const AGG_SIG_ARGS: u32 = 0x800000;

// When enabled, each spend is limited in the number of announcements it is
// allowed to create and assert, to no more than 1000. This includes:
// CREATE_COIN_ANNOUNCEMENT
// CREATE_PUZZLE_ANNOUNCEMENT
// ASSERT_COIN_ANNOUNCEMENT
// ASSERT_PUZZLE_ANNOUNCEMENT
// ASSERT_CONCURRENT_SPEND
// ASSERT_CONCURRENT_PUZZLE
pub const LIMIT_ANNOUNCES: u32 = 0x1000000;

// when this flag is set, the block generator serialization is allowed to
// contain back-references
pub const ALLOW_BACKREFS: u32 = 0x2000000;

pub const MEMPOOL_MODE: u32 = CLVM_MEMPOOL_MODE
    | NO_UNKNOWN_CONDS
    | COND_ARGS_NIL
    | STRICT_ARGS_COUNT
    | NO_RELATIVE_CONDITIONS_ON_EPHEMERAL
    | LIMIT_ANNOUNCES;
