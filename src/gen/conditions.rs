use super::coin_id::compute_coin_id;
use super::condition_sanitizers::{
    parse_amount, parse_create_coin_amount, parse_height, parse_positive_height,
    parse_positive_seconds, parse_seconds, sanitize_announce_msg, sanitize_hash,
};
use super::opcodes::{
    parse_opcode, ConditionOpcode, AGG_SIG_COST, AGG_SIG_ME, AGG_SIG_UNSAFE, ALWAYS_TRUE,
    ASSERT_BEFORE_HEIGHT_ABSOLUTE, ASSERT_BEFORE_HEIGHT_RELATIVE, ASSERT_BEFORE_SECONDS_ABSOLUTE,
    ASSERT_BEFORE_SECONDS_RELATIVE, ASSERT_COIN_ANNOUNCEMENT, ASSERT_CONCURRENT_PUZZLE,
    ASSERT_CONCURRENT_SPEND, ASSERT_HEIGHT_ABSOLUTE, ASSERT_HEIGHT_RELATIVE, ASSERT_MY_AMOUNT,
    ASSERT_MY_BIRTH_HEIGHT, ASSERT_MY_BIRTH_SECONDS, ASSERT_MY_COIN_ID, ASSERT_MY_PARENT_ID,
    ASSERT_MY_PUZZLEHASH, ASSERT_PUZZLE_ANNOUNCEMENT, ASSERT_SECONDS_ABSOLUTE,
    ASSERT_SECONDS_RELATIVE, CREATE_COIN, CREATE_COIN_ANNOUNCEMENT, CREATE_COIN_COST,
    CREATE_PUZZLE_ANNOUNCEMENT, RESERVE_FEE,
};
use super::sanitize_int::sanitize_uint;
use super::validation_error::{first, next, rest, ErrorCode, ValidationErr};
use crate::gen::flags::COND_ARGS_NIL;
use crate::gen::flags::NO_UNKNOWN_CONDS;
use crate::gen::flags::STRICT_ARGS_COUNT;
use crate::gen::validation_error::check_nil;
use chia_protocol::bytes::Bytes32;
use clvmr::allocator::{Allocator, NodePtr, SExp};
use clvmr::cost::Cost;
use clvmr::op_utils::u64_from_bytes;
use clvmr::sha2::{Digest, Sha256};
use std::cmp::{max, min};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

// The structure of conditions, returned from a generator program, is a list,
// where the first element is used, and any additional elements are left unused,
// for future soft-forks.

// The first element is, in turn, a list of all coins being spent.

// Each spend has the following structure:

// (<coin-parent-id> <coin-puzzle-hash> <coin-amount> (CONDITION-LIST ...) ... )

// where ... is possible extra fields that are currently ignored.

// the CONDITIONS-LIST, lists all the conditions for the spend, including
// CREATE_COINs. It has the following format:

// (<condition-opcode> <arg1> <arg2> ...)

// different conditions have different number and types of arguments.

// Example:

// ((<coin-parent-id> <coind-puzzle-hash> <coin-amount> (
//  (
//    (CREATE_COIN <puzzle-hash> <amount>)
//    (ASSERT_HEIGHT_ABSOLUTE <height>)
//  )
// )))

#[derive(PartialEq, Hash, Eq, Debug)]
pub enum Condition {
    // pubkey (48 bytes) and message (<= 1024 bytes)
    AggSigUnsafe(NodePtr, NodePtr),
    AggSigMe(NodePtr, NodePtr),
    // puzzle hash (32 bytes), amount-node, amount integer, hint is an optional
    // hash (32 bytes), may be left as null
    CreateCoin(NodePtr, u64, NodePtr),
    // amount
    ReserveFee(u64),
    // message (<= 1024 bytes)
    CreateCoinAnnouncement(NodePtr),
    CreatePuzzleAnnouncement(NodePtr),
    // announce ID (hash, 32 bytes)
    AssertCoinAnnouncement(NodePtr),
    AssertPuzzleAnnouncement(NodePtr),
    // ensure the specified coin ID is also being spent (hash, 32 bytes)
    AssertConcurrentSpend(NodePtr),
    // ensure that the specified puzzle hash is used by at least one spend
    // (hash, 32 bytes)
    AssertConcurrentPuzzle(NodePtr),
    // ID (hash, 32 bytes)
    AssertMyCoinId(NodePtr),
    AssertMyParentId(NodePtr),
    AssertMyPuzzlehash(NodePtr),
    // amount
    AssertMyAmount(u64),
    // seconds
    AssertMyBirthSeconds(u64),
    // block height
    AssertMyBirthHeight(u32),
    // seconds
    AssertSecondsRelative(u64),
    AssertSecondsAbsolute(u64),
    // block height
    AssertHeightRelative(u32),
    AssertHeightAbsolute(u32),
    // seconds
    AssertBeforeSecondsRelative(u64),
    AssertBeforeSecondsAbsolute(u64),
    // block height
    AssertBeforeHeightRelative(u32),
    AssertBeforeHeightAbsolute(u32),

    // this means the condition is unconditionally true and can be skipped
    Skip,
}

fn maybe_check_args_terminator(
    a: &Allocator,
    arg: NodePtr,
    flags: u32,
) -> Result<(), ValidationErr> {
    if (flags & STRICT_ARGS_COUNT) != 0 {
        check_nil(a, rest(a, arg)?)?;
    }
    Ok(())
}

pub fn parse_args(
    a: &Allocator,
    mut c: NodePtr,
    op: ConditionOpcode,
    flags: u32,
) -> Result<Condition, ValidationErr> {
    match op {
        AGG_SIG_UNSAFE => {
            let pubkey = sanitize_hash(a, first(a, c)?, 48, ErrorCode::InvalidPubkey)?;
            c = rest(a, c)?;
            let message = sanitize_announce_msg(a, first(a, c)?, ErrorCode::InvalidMessage)?;
            // AGG_SIG_UNSAFE takes exactly two parameters
            if (flags & COND_ARGS_NIL) != 0 {
                // make sure there aren't more than two
                check_nil(a, rest(a, c)?)?;
                Ok(Condition::AggSigUnsafe(pubkey, message))
            } else {
                // but the argument list still doesn't need to be terminated by NIL,
                // just any atom will do
                match a.sexp(rest(a, c)?) {
                    SExp::Pair(_, _) => Err(ValidationErr(c, ErrorCode::InvalidCondition)),
                    _ => Ok(Condition::AggSigUnsafe(pubkey, message)),
                }
            }
        }
        AGG_SIG_ME => {
            let pubkey = sanitize_hash(a, first(a, c)?, 48, ErrorCode::InvalidPubkey)?;
            c = rest(a, c)?;
            let message = sanitize_announce_msg(a, first(a, c)?, ErrorCode::InvalidMessage)?;
            // AGG_SIG_ME takes exactly two parameters
            if (flags & COND_ARGS_NIL) != 0 {
                // make sure there aren't more than two
                check_nil(a, rest(a, c)?)?;
                Ok(Condition::AggSigMe(pubkey, message))
            } else {
                // but the argument list still doesn't need to be terminated by NIL,
                // just any atom will do
                match a.sexp(rest(a, c)?) {
                    SExp::Pair(_, _) => Err(ValidationErr(c, ErrorCode::InvalidCondition)),
                    _ => Ok(Condition::AggSigMe(pubkey, message)),
                }
            }
        }
        CREATE_COIN => {
            let puzzle_hash = sanitize_hash(a, first(a, c)?, 32, ErrorCode::InvalidPuzzleHash)?;
            c = rest(a, c)?;
            let amount = parse_create_coin_amount(a, first(a, c)?)?;
            // CREATE_COIN takes an optional 3rd parameter, which is a list of
            // byte buffers (typically a 32 byte hash). We only pull out the
            // first element for now, but may support more in the future.
            // If we find anything else, that's still OK, since garbage is
            // ignored. (unless we're in mempool mode, and the STRICT_ARGS_COUNT
            // flag is set)

            // we always expect one more item, even if it's the zero-terminator
            c = rest(a, c)?;

            // there was another item in the list
            if let Ok(params) = first(a, c) {
                // the item was a cons-box, and params is the left-hand
                // side, the list element
                maybe_check_args_terminator(a, c, flags)?;
                if let Ok(param) = first(a, params) {
                    // pull out the first item (param)
                    if let SExp::Atom(b) = a.sexp(param) {
                        if a.buf(&b).len() <= 32 {
                            return Ok(Condition::CreateCoin(puzzle_hash, amount, param));
                        }
                    }
                }
            } else if (flags & STRICT_ARGS_COUNT) != 0 {
                check_nil(a, c)?;
            }
            Ok(Condition::CreateCoin(puzzle_hash, amount, a.null()))
        }
        RESERVE_FEE => {
            maybe_check_args_terminator(a, c, flags)?;
            let fee = parse_amount(a, first(a, c)?, ErrorCode::ReserveFeeConditionFailed)?;
            Ok(Condition::ReserveFee(fee))
        }
        CREATE_COIN_ANNOUNCEMENT => {
            maybe_check_args_terminator(a, c, flags)?;
            let msg = sanitize_announce_msg(a, first(a, c)?, ErrorCode::InvalidCoinAnnouncement)?;
            Ok(Condition::CreateCoinAnnouncement(msg))
        }
        ASSERT_COIN_ANNOUNCEMENT => {
            maybe_check_args_terminator(a, c, flags)?;
            let id = sanitize_hash(a, first(a, c)?, 32, ErrorCode::AssertCoinAnnouncementFailed)?;
            Ok(Condition::AssertCoinAnnouncement(id))
        }
        CREATE_PUZZLE_ANNOUNCEMENT => {
            maybe_check_args_terminator(a, c, flags)?;
            let msg = sanitize_announce_msg(a, first(a, c)?, ErrorCode::InvalidPuzzleAnnouncement)?;
            Ok(Condition::CreatePuzzleAnnouncement(msg))
        }
        ASSERT_PUZZLE_ANNOUNCEMENT => {
            maybe_check_args_terminator(a, c, flags)?;
            let id = sanitize_hash(
                a,
                first(a, c)?,
                32,
                ErrorCode::AssertPuzzleAnnouncementFailed,
            )?;
            Ok(Condition::AssertPuzzleAnnouncement(id))
        }
        ASSERT_CONCURRENT_SPEND => {
            maybe_check_args_terminator(a, c, flags)?;
            let id = sanitize_hash(a, first(a, c)?, 32, ErrorCode::AssertConcurrentSpendFailed)?;
            Ok(Condition::AssertConcurrentSpend(id))
        }
        ASSERT_CONCURRENT_PUZZLE => {
            maybe_check_args_terminator(a, c, flags)?;
            let id = sanitize_hash(a, first(a, c)?, 32, ErrorCode::AssertConcurrentPuzzleFailed)?;
            Ok(Condition::AssertConcurrentPuzzle(id))
        }
        ASSERT_MY_COIN_ID => {
            maybe_check_args_terminator(a, c, flags)?;
            let id = sanitize_hash(a, first(a, c)?, 32, ErrorCode::AssertMyCoinIdFailed)?;
            Ok(Condition::AssertMyCoinId(id))
        }
        ASSERT_MY_PARENT_ID => {
            maybe_check_args_terminator(a, c, flags)?;
            let id = sanitize_hash(a, first(a, c)?, 32, ErrorCode::AssertMyParentIdFailed)?;
            Ok(Condition::AssertMyParentId(id))
        }
        ASSERT_MY_PUZZLEHASH => {
            maybe_check_args_terminator(a, c, flags)?;
            let id = sanitize_hash(a, first(a, c)?, 32, ErrorCode::AssertMyPuzzlehashFailed)?;
            Ok(Condition::AssertMyPuzzlehash(id))
        }
        ASSERT_MY_AMOUNT => {
            maybe_check_args_terminator(a, c, flags)?;
            let amount = parse_amount(a, first(a, c)?, ErrorCode::AssertMyAmountFailed)?;
            Ok(Condition::AssertMyAmount(amount))
        }
        ASSERT_MY_BIRTH_SECONDS => {
            maybe_check_args_terminator(a, c, flags)?;
            let code = ErrorCode::AssertMyBirthSecondsFailed;
            // both negative values and values exceeding the max are invalid
            let s = match sanitize_uint(a, first(a, c)?, 8, code) {
                Err(ValidationErr(n, ErrorCode::NegativeAmount)) => Err(ValidationErr(n, code)),
                Err(ValidationErr(n, ErrorCode::AmountExceedsMaximum)) => {
                    Err(ValidationErr(n, code))
                }
                Err(r) => Err(r),
                Ok(r) => Ok(u64_from_bytes(r)),
            }?;

            Ok(Condition::AssertMyBirthSeconds(s))
        }
        ASSERT_MY_BIRTH_HEIGHT => {
            maybe_check_args_terminator(a, c, flags)?;
            let code = ErrorCode::AssertMyBirthHeightFailed;
            // both negative values and values exceeding the max are invalid
            let h = match sanitize_uint(a, first(a, c)?, 4, code) {
                Err(ValidationErr(n, ErrorCode::NegativeAmount)) => Err(ValidationErr(n, code)),
                Err(ValidationErr(n, ErrorCode::AmountExceedsMaximum)) => {
                    Err(ValidationErr(n, code))
                }
                Err(r) => Err(r),
                Ok(r) => Ok(u64_from_bytes(r) as u32),
            }?;
            Ok(Condition::AssertMyBirthHeight(h))
        }
        ASSERT_SECONDS_RELATIVE => {
            maybe_check_args_terminator(a, c, flags)?;
            let seconds = parse_seconds(a, first(a, c)?, ErrorCode::AssertSecondsRelative)?;
            Ok(Condition::AssertSecondsRelative(seconds))
        }
        ASSERT_SECONDS_ABSOLUTE => {
            maybe_check_args_terminator(a, c, flags)?;
            let seconds = parse_seconds(a, first(a, c)?, ErrorCode::AssertSecondsAbsolute)?;
            Ok(Condition::AssertSecondsAbsolute(seconds))
        }
        ASSERT_HEIGHT_RELATIVE => {
            maybe_check_args_terminator(a, c, flags)?;
            let code = ErrorCode::AssertHeightRelative;
            match sanitize_uint(a, first(a, c)?, 4, code) {
                // Height is always positive, so a negative requirement is always true,
                Err(ValidationErr(_, ErrorCode::NegativeAmount)) => Ok(Condition::Skip),
                Err(ValidationErr(n, ErrorCode::AmountExceedsMaximum)) => {
                    Err(ValidationErr(n, code))
                }
                Err(r) => Err(r),
                Ok(r) => Ok(Condition::AssertHeightRelative(u64_from_bytes(r) as u32)),
            }
        }
        ASSERT_HEIGHT_ABSOLUTE => {
            maybe_check_args_terminator(a, c, flags)?;
            let height = parse_height(a, first(a, c)?, ErrorCode::AssertHeightAbsolute)?;
            Ok(Condition::AssertHeightAbsolute(height))
        }
        ASSERT_BEFORE_SECONDS_RELATIVE => {
            maybe_check_args_terminator(a, c, flags)?;
            match parse_positive_seconds(a, first(a, c)?, ErrorCode::AssertBeforeSecondsRelative)? {
                None => Ok(Condition::Skip),
                Some(seconds) => Ok(Condition::AssertBeforeSecondsRelative(seconds)),
            }
        }
        ASSERT_BEFORE_SECONDS_ABSOLUTE => {
            maybe_check_args_terminator(a, c, flags)?;
            match parse_positive_seconds(a, first(a, c)?, ErrorCode::AssertBeforeSecondsAbsolute)? {
                None => Ok(Condition::Skip),
                Some(seconds) => Ok(Condition::AssertBeforeSecondsAbsolute(seconds)),
            }
        }
        ASSERT_BEFORE_HEIGHT_RELATIVE => {
            maybe_check_args_terminator(a, c, flags)?;
            match parse_positive_height(a, first(a, c)?, ErrorCode::AssertBeforeHeightRelative)? {
                None => Ok(Condition::Skip),
                Some(height) => Ok(Condition::AssertBeforeHeightRelative(height)),
            }
        }
        ASSERT_BEFORE_HEIGHT_ABSOLUTE => {
            maybe_check_args_terminator(a, c, flags)?;
            match parse_positive_height(a, first(a, c)?, ErrorCode::AssertBeforeHeightAbsolute)? {
                None => Ok(Condition::Skip),
                Some(height) => Ok(Condition::AssertBeforeHeightAbsolute(height)),
            }
        }
        ALWAYS_TRUE => {
            // this condition is always true, we always ignore arguments
            Ok(Condition::Skip)
        }
        _ => Err(ValidationErr(c, ErrorCode::InvalidConditionOpcode)),
    }
}

#[derive(Debug)]
pub struct NewCoin {
    pub puzzle_hash: Bytes32,
    pub amount: u64,
    // the hint is optional. When not provided, this points to null (NodePtr
    // value -1). The hint is not part of the unique identity of a coin, it's not
    // hashed when computing the coin ID
    pub hint: NodePtr,
}

impl Hash for NewCoin {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.puzzle_hash.hash(h);
        self.amount.hash(h);
    }
}

impl Eq for NewCoin {}

impl PartialEq for NewCoin {
    fn eq(&self, lhs: &NewCoin) -> bool {
        self.amount == lhs.amount && self.puzzle_hash == lhs.puzzle_hash
    }
}

// spend flags

// a spend is eligible for deduplication if it does not have any AGG_SIG_ME
// nor AGG_SIG_UNSAFE
pub const ELIGIBLE_FOR_DEDUP: u32 = 1;

// These are all the conditions related directly to a specific spend.
#[derive(Debug)]
pub struct Spend {
    // the parent coin ID of the coin being spent
    pub parent_id: NodePtr,
    // the amount of the coin that's being spent
    pub coin_amount: u64,
    // the puzzle hash of the p
    pub puzzle_hash: NodePtr,
    // the coin ID of the coin being spent. This is computed from parent_id,
    // coin_amount and puzzle_hash
    pub coin_id: Arc<Bytes32>,
    // conditions
    // all these integers are initialized to 0, which also means "no
    // constraint". i.e. a 0 in these conditions are inherently satisified and
    // ignored. 0 (or negative values) are not passed up to the next layer
    // One exception is height_relative, where 0 *is* relevant.
    // the highest height/time conditions (i.e. most strict)
    pub height_relative: Option<u32>,
    pub seconds_relative: u64,
    // the most restrictive ASSERT_BEFORE_HEIGHT_RELATIVE condition (if any)
    // returned by this puzzle. It doesn't make much sense for a puzzle to
    // return multiple of these, but if they do, we only need to care about the
    // lowest one (the most restrictive).
    pub before_height_relative: Option<u32>,
    // the most restrictive ASSERT_BEFORE_SECOND_RELATIVE condition (if any)
    pub before_seconds_relative: Option<u64>,
    // all coins created by this spend. Duplicates are consensus failures
    // if the coin is asserting its birth height or timestamp, these are set
    pub birth_height: Option<u32>,
    pub birth_seconds: Option<u64>,
    pub create_coin: HashSet<NewCoin>,
    // Agg Sig Me conditions
    pub agg_sig_me: Vec<(NodePtr, NodePtr)>,
    // Flags describing properties of this spend. See flags above
    pub flags: u32,
}

// these are all the conditions and properties of a complete spend bundle.
// some conditions that are created by individual spends are aggregated at the
// spend bundle level, like reserve_fee and absolute time locks. Other
// conditions are per spend, like relative time-locks and create coins (because
// they have an implied parent coin ID).
#[derive(Debug, Default)]
pub struct SpendBundleConditions {
    pub spends: Vec<Spend>,
    // conditions
    // all these integers are initialized to 0, which also means "no
    // constraint". i.e. a 0 in these conditions are inherently satisified and
    // ignored. 0 (or negative values) are not passed up to the next layer
    // The sum of all reserve fee conditions
    pub reserve_fee: u64,
    // the highest height/time conditions (i.e. most strict)
    pub height_absolute: u32,
    pub seconds_absolute: u64,
    // Unsafe Agg Sig conditions (i.e. not tied to the spend generating it)
    pub agg_sig_unsafe: Vec<(NodePtr, NodePtr)>,
    // when set, this is the lowest (i.e. most restrictive) of all
    // ASSERT_BEFORE_HEIGHT_ABSOLUTE conditions
    pub before_height_absolute: Option<u32>,
    // ASSERT_BEFORE_SECONDS_ABSOLUTE conditions
    pub before_seconds_absolute: Option<u64>,

    // the cost of conditions (when returned by parse_spends())
    // run_generator() will include the CLVM cost
    // run_block_generator() will include CLVM cost and byte cost (making this
    // the total cost)
    pub cost: u64,

    // the sum of all values of all spent coins
    pub removal_amount: u128,

    // the sum of all amounts of CREATE_COIN conditions
    pub addition_amount: u128,
}

#[derive(Default)]
pub struct ParseState {
    // hashing of the announcements is deferred until parsing is complete. This
    // means less work up-front, in case parsing/validation fails
    announce_coin: HashSet<(Arc<Bytes32>, NodePtr)>,
    announce_puzzle: HashSet<(NodePtr, NodePtr)>,

    // the assert announcements are checked once everything has been parsed and
    // validated.
    assert_coin: HashSet<NodePtr>,
    assert_puzzle: HashSet<NodePtr>,

    // the assert concurrent spend coin IDs are inserted into this set and
    // checked once everything has been parsed.
    assert_concurrent_spend: HashSet<NodePtr>,

    // the assert concurrent puzzle hashes are inserted into this set and
    // checked once everything has been parsed.
    assert_concurrent_puzzle: HashSet<NodePtr>,

    // all coin IDs that have been spent so far. When we parse a spend we also
    // compute the coin ID, and stick it in this set. It's reference counted
    // since it may also be referenced by announcements
    spent_coins: HashSet<Arc<Bytes32>>,

    // for every coin spent, we also store all the puzzle hashes that were
    // spent. Note that these are just the node pointers into the allocator, so
    // there may still be duplicates here. We defer creating a hash set of the
    // actual hashes until the end, and only if there are any puzzle assertions
    spent_puzzles: HashSet<NodePtr>,
}

pub(crate) fn parse_single_spend(
    ret: &mut SpendBundleConditions,
    a: &Allocator,
    state: &mut ParseState,
    mut spend: NodePtr,
    flags: u32,
    max_cost: &mut Cost,
) -> Result<(), ValidationErr> {
    let parent_id = sanitize_hash(a, first(a, spend)?, 32, ErrorCode::InvalidParentId)?;
    spend = rest(a, spend)?;
    let puzzle_hash = sanitize_hash(a, first(a, spend)?, 32, ErrorCode::InvalidPuzzleHash)?;
    spend = rest(a, spend)?;
    let my_amount = parse_amount(a, first(a, spend)?, ErrorCode::InvalidCoinAmount)?;
    let cond = rest(a, spend)?;
    let amount_buf = a.atom(first(a, spend)?);
    let coin_id = Arc::new(compute_coin_id(a, parent_id, puzzle_hash, amount_buf));

    if !state.spent_coins.insert(coin_id.clone()) {
        // if this coin ID has already been added to this set, it's a double
        // spend
        return Err(ValidationErr(spend, ErrorCode::DoubleSpend));
    }

    state.spent_puzzles.insert(puzzle_hash);

    ret.removal_amount += my_amount as u128;

    let coin_spend = Spend {
        parent_id,
        coin_amount: my_amount,
        puzzle_hash,
        coin_id,
        height_relative: None,
        seconds_relative: 0,
        before_height_relative: None,
        before_seconds_relative: None,
        birth_height: None,
        birth_seconds: None,
        create_coin: HashSet::new(),
        agg_sig_me: Vec::new(),
        // assume it's eligible until we see an agg-sig condition
        flags: ELIGIBLE_FOR_DEDUP,
    };

    let iter = first(a, cond)?;
    parse_conditions(a, ret, state, coin_spend, iter, flags, max_cost)
}

pub fn parse_conditions(
    a: &Allocator,
    ret: &mut SpendBundleConditions,
    state: &mut ParseState,
    mut spend: Spend,
    mut iter: NodePtr,
    flags: u32,
    max_cost: &mut Cost,
) -> Result<(), ValidationErr> {
    while let Some((mut c, next)) = next(a, iter)? {
        iter = next;
        let op = match parse_opcode(a, first(a, c)?, flags) {
            None => {
                // in strict mode we don't allow unknown conditions
                if (flags & NO_UNKNOWN_CONDS) != 0 {
                    return Err(ValidationErr(c, ErrorCode::InvalidConditionOpcode));
                }
                // in non-strict mode, we just ignore unknown conditions
                continue;
            }
            Some(v) => v,
        };

        // subtract the max_cost based on the current condition
        // in case we exceed the limit, we want to fail as early as possible
        match op {
            CREATE_COIN => {
                if *max_cost < CREATE_COIN_COST {
                    return Err(ValidationErr(c, ErrorCode::CostExceeded));
                }
                *max_cost -= CREATE_COIN_COST;
            }
            AGG_SIG_UNSAFE | AGG_SIG_ME => {
                if *max_cost < AGG_SIG_COST {
                    return Err(ValidationErr(c, ErrorCode::CostExceeded));
                }
                *max_cost -= AGG_SIG_COST;
            }
            _ => (),
        }
        c = rest(a, c)?;
        let cva = parse_args(a, c, op, flags)?;
        match cva {
            Condition::ReserveFee(limit) => {
                // reserve fees are accumulated
                ret.reserve_fee = ret
                    .reserve_fee
                    .checked_add(limit)
                    .ok_or(ValidationErr(c, ErrorCode::ReserveFeeConditionFailed))?;
            }
            Condition::CreateCoin(ph, amount, hint) => {
                let new_coin = NewCoin {
                    puzzle_hash: a.atom(ph).into(),
                    amount,
                    hint,
                };
                if !spend.create_coin.insert(new_coin) {
                    return Err(ValidationErr(c, ErrorCode::DuplicateOutput));
                }
                ret.addition_amount += amount as u128;
            }
            Condition::AssertSecondsRelative(s) => {
                // keep the most strict condition. i.e. the highest limit
                spend.seconds_relative = max(spend.seconds_relative, s);
                if let Some(bs) = spend.before_seconds_relative {
                    if bs <= spend.seconds_relative {
                        // this spend bundle requres to be spent *before* a
                        // timestamp and also *after* a timestamp that's the
                        // same or later. that's impossible.
                        return Err(ValidationErr(
                            c,
                            ErrorCode::ImpossibleSecondsRelativeConstraints,
                        ));
                    }
                }
            }
            Condition::AssertSecondsAbsolute(s) => {
                // keep the most strict condition. i.e. the highest limit
                ret.seconds_absolute = max(ret.seconds_absolute, s);
            }
            Condition::AssertHeightRelative(h) => {
                // keep the most strict condition. i.e. the highest limit
                spend.height_relative = Some(max(spend.height_relative.unwrap_or(0), h));
                if let Some(bs) = spend.before_height_relative {
                    if bs <= h {
                        // this spend bundle requres to be spent *before* a
                        // height and also *after* a height that's the
                        // same or later. that's impossible.
                        return Err(ValidationErr(
                            c,
                            ErrorCode::ImpossibleHeightRelativeConstraints,
                        ));
                    }
                }
            }
            Condition::AssertHeightAbsolute(h) => {
                // keep the most strict condition. i.e. the highest limit
                ret.height_absolute = max(ret.height_absolute, h);
            }
            Condition::AssertBeforeSecondsRelative(s) => {
                // keep the most strict condition. i.e. the lowest limit
                if let Some(existing) = spend.before_seconds_relative {
                    spend.before_seconds_relative = Some(min(existing, s));
                } else {
                    spend.before_seconds_relative = Some(s);
                }
                if s <= spend.seconds_relative {
                    // this spend bundle requres to be spent *before* a
                    // timestamp and also *after* a timestamp that's the
                    // same or later. that's impossible.
                    return Err(ValidationErr(
                        c,
                        ErrorCode::ImpossibleSecondsRelativeConstraints,
                    ));
                }
            }
            Condition::AssertBeforeSecondsAbsolute(s) => {
                // keep the most strict condition. i.e. the lowest limit
                if let Some(existing) = ret.before_seconds_absolute {
                    ret.before_seconds_absolute = Some(min(existing, s));
                } else {
                    ret.before_seconds_absolute = Some(s);
                }
            }
            Condition::AssertBeforeHeightRelative(h) => {
                // keep the most strict condition. i.e. the lowest limit
                if let Some(existing) = spend.before_height_relative {
                    spend.before_height_relative = Some(min(existing, h));
                } else {
                    spend.before_height_relative = Some(h);
                }
                if let Some(hr) = spend.height_relative {
                    if h <= hr {
                        // this spend bundle requres to be spent *before* a
                        // height and also *after* a height that's the
                        // same or later. that's impossible.
                        return Err(ValidationErr(
                            c,
                            ErrorCode::ImpossibleHeightRelativeConstraints,
                        ));
                    }
                }
            }
            Condition::AssertBeforeHeightAbsolute(h) => {
                // keep the most strict condition. i.e. the lowest limit
                if let Some(existing) = ret.before_height_absolute {
                    ret.before_height_absolute = Some(min(existing, h));
                } else {
                    ret.before_height_absolute = Some(h);
                }
            }
            Condition::AssertMyCoinId(id) => {
                if a.atom(id) != (*spend.coin_id).as_ref() {
                    return Err(ValidationErr(c, ErrorCode::AssertMyCoinIdFailed));
                }
            }
            Condition::AssertMyAmount(amount) => {
                if amount != spend.coin_amount {
                    return Err(ValidationErr(c, ErrorCode::AssertMyAmountFailed));
                }
            }
            Condition::AssertMyBirthSeconds(s) => {
                // if this spend already has a birth_seconds assertion, it's an
                // error if it's different from the new birth assertion. One of
                // them must be false
                if spend.birth_seconds.map(|v| v == s) == Some(false) {
                    return Err(ValidationErr(c, ErrorCode::AssertMyBirthSecondsFailed));
                }
                spend.birth_seconds = Some(s);
            }
            Condition::AssertMyBirthHeight(h) => {
                // if this spend already has a birth_height assertion, it's an
                // error if it's different from the new birth assertion. One of
                // them must be false
                if spend.birth_height.map(|v| v == h) == Some(false) {
                    return Err(ValidationErr(c, ErrorCode::AssertMyBirthHeightFailed));
                }
                spend.birth_height = Some(h);
            }
            Condition::AssertMyParentId(id) => {
                if a.atom(id) != a.atom(spend.parent_id) {
                    return Err(ValidationErr(c, ErrorCode::AssertMyParentIdFailed));
                }
            }
            Condition::AssertMyPuzzlehash(hash) => {
                if a.atom(hash) != a.atom(spend.puzzle_hash) {
                    return Err(ValidationErr(c, ErrorCode::AssertMyPuzzlehashFailed));
                }
            }
            Condition::CreateCoinAnnouncement(msg) => {
                state.announce_coin.insert((spend.coin_id.clone(), msg));
            }
            Condition::CreatePuzzleAnnouncement(msg) => {
                state.announce_puzzle.insert((spend.puzzle_hash, msg));
            }
            Condition::AssertCoinAnnouncement(msg) => {
                state.assert_coin.insert(msg);
            }
            Condition::AssertPuzzleAnnouncement(msg) => {
                state.assert_puzzle.insert(msg);
            }
            Condition::AssertConcurrentSpend(id) => {
                state.assert_concurrent_spend.insert(id);
            }
            Condition::AssertConcurrentPuzzle(id) => {
                state.assert_concurrent_puzzle.insert(id);
            }
            Condition::AggSigMe(pk, msg) => {
                spend.agg_sig_me.push((pk, msg));
                spend.flags &= !ELIGIBLE_FOR_DEDUP;
            }
            Condition::AggSigUnsafe(pk, msg) => {
                ret.agg_sig_unsafe.push((pk, msg));
                spend.flags &= !ELIGIBLE_FOR_DEDUP;
            }
            Condition::Skip => {}
        }
    }

    ret.spends.push(spend);
    Ok(())
}

// This function parses, and validates aspects of, the above structure and
// returns a list of all spends, along with all conditions, organized by
// condition op-code
pub fn parse_spends(
    a: &Allocator,
    spends: NodePtr,
    max_cost: Cost,
    flags: u32,
) -> Result<SpendBundleConditions, ValidationErr> {
    let mut ret = SpendBundleConditions::default();
    let mut state = ParseState::default();

    let mut cost_left = max_cost;

    let mut iter = first(a, spends)?;
    while let Some((spend, next)) = next(a, iter)? {
        iter = next;
        // cost_left is passed in as a mutable reference and decremented by the
        // cost of the condition (if it has a cost). This let us fail as early
        // as possible if cost is exceeded
        // this function adds the spend to the passed-in ret
        // as well as updates it with any conditions
        parse_single_spend(&mut ret, a, &mut state, spend, flags, &mut cost_left)?;
    }

    if ret.removal_amount < ret.addition_amount {
        // The sum of removal amounts must not be less than the sum of addition
        // amounts
        return Err(ValidationErr(spends, ErrorCode::MintingCoin));
    }

    if ret.removal_amount - ret.addition_amount < ret.reserve_fee as u128 {
        // the actual fee is lower than the reserved fee
        return Err(ValidationErr(spends, ErrorCode::ReserveFeeConditionFailed));
    }

    if let Some(bh) = ret.before_height_absolute {
        if bh <= ret.height_absolute {
            // this spend bundle requres to be spent *before* a
            // height and also *after* a height that's the
            // same or later. that's impossible.
            return Err(ValidationErr(
                spends,
                ErrorCode::ImpossibleHeightAbsoluteConstraints,
            ));
        }
    }

    if let Some(bs) = ret.before_seconds_absolute {
        if bs <= ret.seconds_absolute {
            // this spend bundle requres to be spent *before* a
            // timestamp and also *after* a timestamp that's the
            // same or later. that's impossible.
            return Err(ValidationErr(
                spends,
                ErrorCode::ImpossibleSecondsAbsoluteConstraints,
            ));
        }
    }

    // check concurrent spent assertions
    for coin_id in state.assert_concurrent_spend {
        if !state.spent_coins.contains(&Bytes32::from(a.atom(coin_id))) {
            return Err(ValidationErr(
                coin_id,
                ErrorCode::AssertConcurrentSpendFailed,
            ));
        }
    }

    if !state.assert_concurrent_puzzle.is_empty() {
        let mut spent_phs = HashSet::<Bytes32>::new();

        // expand all the spent puzzle hashes into a set, to allow
        // fast lookups of all assertions
        for ph in state.spent_puzzles {
            spent_phs.insert(a.atom(ph).into());
        }

        for puzzle_assert in state.assert_concurrent_puzzle {
            if !spent_phs.contains(&a.atom(puzzle_assert).into()) {
                return Err(ValidationErr(
                    puzzle_assert,
                    ErrorCode::AssertConcurrentPuzzleFailed,
                ));
            }
        }
    }

    // check all the assert announcements
    // if there are no asserts, there is no need to hash all the announcements
    if !state.assert_coin.is_empty() {
        let mut announcements = HashSet::<Bytes32>::new();

        for (coin_id, announce) in state.announce_coin {
            let mut hasher = Sha256::new();
            hasher.update(*coin_id);
            hasher.update(a.atom(announce));
            announcements.insert(hasher.finalize().as_slice().into());
        }

        for coin_assert in state.assert_coin {
            if !announcements.contains(&a.atom(coin_assert).into()) {
                return Err(ValidationErr(
                    coin_assert,
                    ErrorCode::AssertCoinAnnouncementFailed,
                ));
            }
        }
    }

    if !state.assert_puzzle.is_empty() {
        let mut announcements = HashSet::<Bytes32>::new();

        for (puzzle_hash, announce) in state.announce_puzzle {
            let mut hasher = Sha256::new();
            hasher.update(a.atom(puzzle_hash));
            hasher.update(a.atom(announce));
            announcements.insert(hasher.finalize().as_slice().into());
        }

        for puzzle_assert in state.assert_puzzle {
            if !announcements.contains(&a.atom(puzzle_assert).into()) {
                return Err(ValidationErr(
                    puzzle_assert,
                    ErrorCode::AssertPuzzleAnnouncementFailed,
                ));
            }
        }
    }

    // TODO: there may be more failures that can be detected early here, for
    // example an assert-my-birth-height that's incompatible assert-height or
    // assert-before-height. Same thing for the seconds counterpart

    ret.cost = max_cost - cost_left;

    Ok(ret)
}

#[cfg(test)]
fn u64_to_bytes(n: u64) -> Vec<u8> {
    let mut buf = Vec::<u8>::new();
    buf.extend_from_slice(&n.to_be_bytes());
    if (buf[0] & 0x80) != 0 {
        buf.insert(0, 0);
    } else {
        while buf.len() > 1 && buf[0] == 0 && (buf[1] & 0x80) == 0 {
            buf.remove(0);
        }
    }
    buf
}
#[cfg(test)]
use crate::gen::flags::ENABLE_ASSERT_BEFORE;
#[cfg(test)]
use clvmr::node::Node;
#[cfg(test)]
use clvmr::number::{ptr_from_number, Number};
#[cfg(test)]
use clvmr::serde::node_to_bytes;
#[cfg(test)]
use hex::FromHex;
#[cfg(test)]
use num_traits::Num;
#[cfg(test)]
use rstest::rstest;
#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
const H1: &[u8; 32] = &[
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
];
#[cfg(test)]
const H2: &[u8; 32] = &[
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
];

#[cfg(test)]
const LONG_VEC: &[u8; 33] = &[
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3,
];

#[cfg(test)]
const PUBKEY: &[u8; 48] = &[
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
];
#[cfg(test)]
const MSG1: &[u8; 13] = &[3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3];
#[cfg(test)]
const MSG2: &[u8; 19] = &[4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4];

#[cfg(test)]
const LONGMSG: &[u8; 1025] = &[
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    4,
];

#[cfg(test)]
fn hash_buf(b1: &[u8], b2: &[u8]) -> Vec<u8> {
    let mut ctx = Sha256::new();
    ctx.update(b1);
    ctx.update(b2);
    ctx.finalize().to_vec()
}

#[cfg(test)]
fn test_coin_id(parent_id: &[u8; 32], puzzle_hash: &[u8; 32], amount: u64) -> Bytes32 {
    let mut hasher = Sha256::new();
    hasher.update(parent_id);
    hasher.update(puzzle_hash);
    let buf = u64_to_bytes(amount);
    hasher.update(&buf);
    hasher.finalize().as_slice().into()
}

// this is a very simple parser. It does not handle errors, because it's only
// meant for tests
// * redundant white space is not supported.
// * lists are not supported, only cons boxes
// * cons boxes may not be terminated by ")". They are terminated implicitly after
//   the second atom.
// * ) means nil
// * substitutions for test values can be done with {name} in the input string.
// * arbitrary substitutions can be made with a callback and {} in the intput
//   string
// Example:
// (1 (2 (3 ) means: (1 . (2 . (3 . ())))
// and:

#[cfg(test)]
fn parse_list_impl(
    a: &mut Allocator,
    input: &str,
    callback: &Option<fn(&mut Allocator) -> NodePtr>,
    subs: &HashMap<&'static str, NodePtr>,
) -> (NodePtr, usize) {
    // skip whitespace
    if input.starts_with(" ") {
        let (n, skip) = parse_list_impl(a, &input[1..], callback, subs);
        return (n, skip + 1);
    }

    if input.starts_with(")") {
        (a.null(), 1)
    } else if input.starts_with("(") {
        let (first, step1) = parse_list_impl(a, &input[1..], callback, subs);
        let (rest, step2) = parse_list_impl(a, &input[(1 + step1)..], callback, subs);
        (a.new_pair(first, rest).unwrap(), 1 + step1 + step2)
    } else if input.starts_with("{") {
        // substitute '{X}' tokens with our test hashes and messages
        // this keeps the test cases a lot simpler
        let var = input[1..].split_once("}").unwrap().0;

        let ret = match var {
            "" => callback.unwrap()(a),
            _ => *subs.get(var).unwrap(),
        };
        (ret, var.len() + 2)
    } else if input.starts_with("0x") {
        let v = input.split_once(" ").unwrap().0;

        let buf = Vec::from_hex(v.strip_prefix("0x").unwrap()).unwrap();
        (a.new_atom(&buf).unwrap(), v.len() + 1)
    } else if input.starts_with("-") || "0123456789".contains(input.get(0..1).unwrap()) {
        let v = input.split_once(" ").unwrap().0;
        let num = Number::from_str_radix(v, 10).unwrap();
        (ptr_from_number(a, &num).unwrap(), v.len() + 1)
    } else {
        panic!("atom not supported \"{}\"", input);
    }
}

#[cfg(test)]
fn parse_list(
    a: &mut Allocator,
    input: &str,
    callback: &Option<fn(&mut Allocator) -> NodePtr>,
) -> NodePtr {
    // all substitutions are allocated up-front in order to have them all use
    // the same atom in the CLVM structure. This is to cover cases where
    // conditions may be deduplicated based on the NodePtr value, when they
    // shouldn't be. The AggSig conditions are stored with NodePtr values, but
    // should never be deduplicated.
    let mut subs = HashMap::<&'static str, NodePtr>::new();

    // hashes
    subs.insert("h1", a.new_atom(H1).unwrap());
    subs.insert("h2", a.new_atom(H2).unwrap());
    subs.insert("long", a.new_atom(LONG_VEC).unwrap());
    // public key
    subs.insert("pubkey", a.new_atom(PUBKEY).unwrap());
    // announce/aggsig messages
    subs.insert("msg1", a.new_atom(MSG1).unwrap());
    subs.insert("msg2", a.new_atom(MSG2).unwrap());
    subs.insert("longmsg", a.new_atom(LONGMSG).unwrap());
    // coin IDs
    subs.insert("coin11", a.new_atom(&test_coin_id(H1, H1, 123)).unwrap());
    subs.insert("coin12", a.new_atom(&test_coin_id(H1, H2, 123)).unwrap());
    subs.insert("coin21", a.new_atom(&test_coin_id(H2, H1, 123)).unwrap());
    subs.insert("coin22", a.new_atom(&test_coin_id(H2, H2, 123)).unwrap());
    subs.insert(
        "coin12_h2_42",
        a.new_atom(&test_coin_id(
            &test_coin_id(H1, H2, 123).as_ref().try_into().unwrap(),
            H2,
            42,
        ))
        .unwrap(),
    );
    // coin announcements
    subs.insert(
        "c11",
        a.new_atom(&hash_buf(&test_coin_id(H1, H2, 123), MSG1))
            .unwrap(),
    );
    subs.insert(
        "c21",
        a.new_atom(&hash_buf(&test_coin_id(H2, H2, 123), MSG1))
            .unwrap(),
    );
    subs.insert(
        "c12",
        a.new_atom(&hash_buf(&test_coin_id(H1, H2, 123), MSG2))
            .unwrap(),
    );
    subs.insert(
        "c22",
        a.new_atom(&hash_buf(&test_coin_id(H2, H2, 123), MSG2))
            .unwrap(),
    );
    // puzzle announcements
    subs.insert("p11", a.new_atom(&hash_buf(H1, MSG1)).unwrap());
    subs.insert("p21", a.new_atom(&hash_buf(H2, MSG1)).unwrap());
    subs.insert("p12", a.new_atom(&hash_buf(H1, MSG2)).unwrap());
    subs.insert("p22", a.new_atom(&hash_buf(H2, MSG2)).unwrap());

    let (n, count) = parse_list_impl(a, input, callback, &subs);
    assert_eq!(&input[count..], "");
    n
}

// The callback can be used for arbitrary substitutions using {} in the input
// string. Since the parser is recursive and simple, large structures have to be
// constructed this way
#[cfg(test)]
fn cond_test_cb(
    input: &str,
    flags: u32,
    callback: Option<fn(&mut Allocator) -> NodePtr>,
) -> Result<(Allocator, SpendBundleConditions), ValidationErr> {
    let mut a = Allocator::new();

    println!("input: {}", input);

    let n = parse_list(&mut a, &input, &callback);
    for c in node_to_bytes(&Node::new(&a, n)).unwrap() {
        print!("{:02x}", c);
    }
    println!();
    match parse_spends(&a, n, 11000000000, flags) {
        Ok(list) => {
            for n in &list.spends {
                println!("{:?}", n);
            }
            Ok((a, list))
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
const MEMPOOL_MODE: u32 =
    COND_ARGS_NIL | STRICT_ARGS_COUNT | NO_UNKNOWN_CONDS | ENABLE_ASSERT_BEFORE;

#[cfg(test)]
fn cond_test(input: &str) -> Result<(Allocator, SpendBundleConditions), ValidationErr> {
    // by default, run all tests in strict mempool mode
    cond_test_cb(input, MEMPOOL_MODE, None)
}

#[cfg(test)]
fn cond_test_flag(
    input: &str,
    flags: u32,
) -> Result<(Allocator, SpendBundleConditions), ValidationErr> {
    cond_test_cb(input, flags, None)
}

#[test]
fn test_invalid_condition_list1() {
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (8 )))").unwrap_err().1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_invalid_condition_list2() {
    assert_eq!(
        cond_test("((({h1} ({h2} (123 ((8 ))))").unwrap_err().1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_invalid_condition_args_terminator() {
    // we only look at the condition arguments the condition expects, any
    // additional arguments are ignored, including the terminator
    // ASSERT_SECONDS_RELATIVE
    let (a, conds) = cond_test_flag("((({h1} ({h2} (123 (((80 (50 8 ))))", 0).unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);

    assert_eq!(spend.seconds_relative, 50);
}

#[test]
fn test_invalid_condition_args_terminator_mempool() {
    // ASSERT_SECONDS_RELATIVE
    // in mempool mode, the argument list must be properly terminated
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((80 (50 8 ))))")
            .unwrap_err()
            .1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_invalid_condition_list_terminator() {
    // ASSERT_SECONDS_RELATIVE
    let (a, conds) = cond_test_flag("((({h1} ({h2} (123 (((80 (50 8 ))))", 0).unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);

    assert_eq!(spend.seconds_relative, 50);
}

#[test]
fn test_invalid_condition_list_terminator_mempool() {
    // ASSERT_SECONDS_RELATIVE
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((80 (50 8 ))))")
            .unwrap_err()
            .1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_invalid_condition_short_list_terminator() {
    // ASSERT_SECONDS_RELATIVE
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((80 8 ))))").unwrap_err().1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_invalid_spend_list1() {
    assert_eq!(
        cond_test("(8 )").unwrap_err().1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_invalid_spend_list2() {
    assert_eq!(
        cond_test("((8 ))").unwrap_err().1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_invalid_spend_list_terminator() {
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (()) 8 ))").unwrap_err().1,
        ErrorCode::InvalidCondition
    );
}

#[cfg(test)]
#[rstest]
#[case(ASSERT_SECONDS_ABSOLUTE, "104")]
#[case(ASSERT_SECONDS_RELATIVE, "101")]
#[case(ASSERT_HEIGHT_RELATIVE, "101")]
#[case(ASSERT_HEIGHT_ABSOLUTE, "100")]
#[case(ASSERT_BEFORE_SECONDS_ABSOLUTE, "104")]
#[case(ASSERT_BEFORE_SECONDS_RELATIVE, "101")]
#[case(ASSERT_BEFORE_HEIGHT_RELATIVE, "101")]
#[case(ASSERT_BEFORE_HEIGHT_ABSOLUTE, "100")]
#[case(RESERVE_FEE, "100")]
#[case(CREATE_COIN_ANNOUNCEMENT, "{msg1}")]
#[case(ASSERT_COIN_ANNOUNCEMENT, "{c11}")]
#[case(CREATE_PUZZLE_ANNOUNCEMENT, "{msg1}")]
#[case(ASSERT_PUZZLE_ANNOUNCEMENT, "{p21}")]
#[case(ASSERT_MY_AMOUNT, "123")]
#[case(ASSERT_MY_BIRTH_SECONDS, "123")]
#[case(ASSERT_MY_BIRTH_HEIGHT, "123")]
#[case(ASSERT_MY_COIN_ID, "{coin12}")]
#[case(ASSERT_MY_PARENT_ID, "{h1}")]
#[case(ASSERT_MY_PUZZLEHASH, "{h2}")]
#[case(CREATE_COIN, "{h2} (42 (({h1})")]
#[case(AGG_SIG_UNSAFE, "{pubkey} ({msg1}")]
#[case(AGG_SIG_ME, "{pubkey} ({msg1}")]
#[case(ASSERT_CONCURRENT_SPEND, "{coin12}")]
#[case(ASSERT_CONCURRENT_PUZZLE, "{h2}")]
fn test_extra_arg_mempool(#[case] condition: ConditionOpcode, #[case] arg: &str) {
    // extra args are disallowed in mempool mode
    assert_eq!(
        cond_test_flag(
            &format!(
                "((({{h1}} ({{h2}} (123 ((({} ({} ( 1337 )))))",
                condition as u8, arg
            ),
            STRICT_ARGS_COUNT | ENABLE_ASSERT_BEFORE
        )
        .unwrap_err()
        .1,
        ErrorCode::InvalidCondition
    );
}

#[cfg(test)]
#[rstest]
#[case(ASSERT_SECONDS_ABSOLUTE, "104", "", |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.seconds_absolute, 104))]
#[case(ASSERT_SECONDS_RELATIVE, "101", "", |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.seconds_relative, 101))]
#[case(ASSERT_HEIGHT_RELATIVE, "101", "", |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.height_relative, Some(101)))]
#[case(ASSERT_HEIGHT_ABSOLUTE, "100", "", |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.height_absolute, 100))]
#[case(ASSERT_BEFORE_SECONDS_ABSOLUTE, "104", "", |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.before_seconds_absolute, Some(104)))]
#[case(ASSERT_BEFORE_SECONDS_RELATIVE, "101", "", |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.before_seconds_relative, Some(101)))]
#[case(ASSERT_BEFORE_HEIGHT_RELATIVE, "101", "", |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.before_height_relative, Some(101)))]
#[case(ASSERT_BEFORE_HEIGHT_ABSOLUTE, "100", "", |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.before_height_absolute, Some(100)))]
#[case(RESERVE_FEE, "100", "", |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.reserve_fee, 100))]
#[case(CREATE_COIN_ANNOUNCEMENT, "{msg1}", "((61 ({c11} )", |_: &SpendBundleConditions, _: &Spend| {})]
#[case(ASSERT_COIN_ANNOUNCEMENT, "{c11}", "((60 ({msg1} )", |_: &SpendBundleConditions, _: &Spend| {})]
#[case(CREATE_PUZZLE_ANNOUNCEMENT, "{msg1}", "((63 ({p21} )", |_: &SpendBundleConditions, _: &Spend| {})]
#[case(ASSERT_PUZZLE_ANNOUNCEMENT, "{p21}", "((62 ({msg1} )", |_: &SpendBundleConditions, _: &Spend| {})]
#[case(ASSERT_MY_AMOUNT, "123", "", |_: &SpendBundleConditions, _: &Spend| {})]
#[case(ASSERT_MY_BIRTH_SECONDS, "123", "", |_: &SpendBundleConditions, s: &Spend| { assert_eq!(s.birth_seconds, Some(123)); })]
#[case(ASSERT_MY_BIRTH_HEIGHT, "123", "", |_: &SpendBundleConditions, s: &Spend| { assert_eq!(s.birth_height, Some(123)); })]
#[case(ASSERT_MY_COIN_ID, "{coin12}", "", |_: &SpendBundleConditions, _: &Spend| {})]
#[case(ASSERT_MY_PARENT_ID, "{h1}", "", |_: &SpendBundleConditions, _: &Spend| {})]
#[case(ASSERT_MY_PUZZLEHASH, "{h2}", "", |_: &SpendBundleConditions, _: &Spend| {})]
#[case(ASSERT_CONCURRENT_SPEND, "{coin12}", "", |_: &SpendBundleConditions, _: &Spend| {})]
#[case(ASSERT_CONCURRENT_PUZZLE, "{h2}", "", |_: &SpendBundleConditions, _: &Spend| {})]
fn test_extra_arg(
    #[case] condition: ConditionOpcode,
    #[case] arg: &str,
    #[case] extra_cond: &str,
    #[case] test: impl Fn(&SpendBundleConditions, &Spend),
) {
    // extra args are ignored
    let (a, conds) = cond_test_flag(
        &format!(
            "((({{h1}} ({{h2}} (123 ((({} ({} ( 1337 ) {} ))))",
            condition as u8, arg, extra_cond
        ),
        ENABLE_ASSERT_BEFORE,
    )
    .unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);

    test(&conds, &spend);
}

#[cfg(test)]
#[rstest]
#[case(ASSERT_SECONDS_ABSOLUTE, "104", |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.seconds_absolute, 104))]
#[case(ASSERT_SECONDS_ABSOLUTE, "-1", |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.seconds_absolute, 0))]
#[case(ASSERT_SECONDS_RELATIVE, "101", |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.seconds_relative, 101))]
#[case(ASSERT_SECONDS_RELATIVE, "-1", |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.seconds_relative, 0))]
#[case(ASSERT_HEIGHT_RELATIVE, "101", |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.height_relative, Some(101)))]
#[case(ASSERT_HEIGHT_RELATIVE, "-1", |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.height_relative, None))]
#[case(ASSERT_HEIGHT_ABSOLUTE, "100", |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.height_absolute, 100))]
#[case(ASSERT_HEIGHT_ABSOLUTE, "-1", |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.height_absolute, 0))]
#[case(ASSERT_BEFORE_SECONDS_ABSOLUTE, "104", |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.before_seconds_absolute, Some(104)))]
#[case(ASSERT_BEFORE_SECONDS_RELATIVE, "101", |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.before_seconds_relative, Some(101)))]
#[case(ASSERT_BEFORE_HEIGHT_RELATIVE, "101", |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.before_height_relative, Some(101)))]
#[case(ASSERT_BEFORE_HEIGHT_RELATIVE, "0", |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.before_height_relative, Some(0)))]
#[case(ASSERT_BEFORE_HEIGHT_ABSOLUTE, "100", |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.before_height_absolute, Some(100)))]
#[case(RESERVE_FEE, "100", |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.reserve_fee, 100))]
#[case(ASSERT_MY_AMOUNT, "123", |_: &SpendBundleConditions, _: &Spend| {})]
#[case(ASSERT_MY_BIRTH_SECONDS, "123", |_: &SpendBundleConditions, s: &Spend| { assert_eq!(s.birth_seconds, Some(123)); })]
#[case(ASSERT_MY_BIRTH_HEIGHT, "123", |_: &SpendBundleConditions, s: &Spend| { assert_eq!(s.birth_height, Some(123)); })]
#[case(ASSERT_MY_COIN_ID, "{coin12}", |_: &SpendBundleConditions, _: &Spend| {})]
#[case(ASSERT_MY_PARENT_ID, "{h1}", |_: &SpendBundleConditions, _: &Spend| {})]
#[case(ASSERT_MY_PUZZLEHASH, "{h2}", |_: &SpendBundleConditions, _: &Spend| {})]
#[case(ASSERT_CONCURRENT_SPEND, "{coin12}", |_: &SpendBundleConditions, _: &Spend| {})]
#[case(ASSERT_CONCURRENT_PUZZLE, "{h2}", |_: &SpendBundleConditions, _: &Spend| {})]
fn test_single_condition(
    #[case] condition: ConditionOpcode,
    #[case] arg: &str,
    #[case] test: impl Fn(&SpendBundleConditions, &Spend),
) {
    let (a, conds) = cond_test(&format!(
        "((({{h1}} ({{h2}} (123 ((({} ({} )))))",
        condition as u8, arg
    ))
    .unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);

    test(&conds, &spend);
}

#[cfg(test)]
#[rstest]
#[case(ASSERT_BEFORE_SECONDS_ABSOLUTE, "0x010000000000000000")]
#[case(ASSERT_BEFORE_SECONDS_RELATIVE, "0x010000000000000000")]
#[case(ASSERT_BEFORE_HEIGHT_ABSOLUTE, "0x0100000000")]
#[case(ASSERT_BEFORE_HEIGHT_RELATIVE, "0x0100000000")]
#[case(ASSERT_SECONDS_ABSOLUTE, "-1")]
#[case(ASSERT_SECONDS_RELATIVE, "-1")]
#[case(ASSERT_HEIGHT_ABSOLUTE, "-1")]
#[case(ASSERT_HEIGHT_RELATIVE, "-1")]
fn test_single_condition_no_op(#[case] condition: ConditionOpcode, #[case] value: &str) {
    let (_, conds) = cond_test(&format!(
        "((({{h1}} ({{h2}} (123 ((({} ({} )))))",
        condition as u8, value
    ))
    .unwrap();

    assert_eq!(conds.height_absolute, 0);
    assert_eq!(conds.seconds_absolute, 0);
    assert_eq!(conds.before_height_absolute, None);
    assert_eq!(conds.before_seconds_absolute, None);
    let spend = &conds.spends[0];
    assert_eq!(spend.before_height_relative, None);
    assert_eq!(spend.before_seconds_relative, None);
    assert_eq!(spend.height_relative, None);
    assert_eq!(spend.seconds_relative, 0);
}

#[cfg(test)]
#[rstest]
#[case(
    ASSERT_SECONDS_ABSOLUTE,
    "0x010000000000000000",
    ErrorCode::AssertSecondsAbsolute
)]
#[case(
    ASSERT_SECONDS_RELATIVE,
    "0x010000000000000000",
    ErrorCode::AssertSecondsRelative
)]
#[case(
    ASSERT_HEIGHT_ABSOLUTE,
    "0x0100000000",
    ErrorCode::AssertHeightAbsolute
)]
#[case(
    ASSERT_HEIGHT_RELATIVE,
    "0x0100000000",
    ErrorCode::AssertHeightRelative
)]
#[case(
    ASSERT_BEFORE_SECONDS_ABSOLUTE,
    "-1",
    ErrorCode::AssertBeforeSecondsAbsolute
)]
#[case(
    ASSERT_BEFORE_SECONDS_ABSOLUTE,
    "0",
    ErrorCode::ImpossibleSecondsAbsoluteConstraints
)]
#[case(
    ASSERT_BEFORE_SECONDS_RELATIVE,
    "-1",
    ErrorCode::AssertBeforeSecondsRelative
)]
#[case(
    ASSERT_BEFORE_SECONDS_RELATIVE,
    "0",
    ErrorCode::ImpossibleSecondsRelativeConstraints
)]
#[case(
    ASSERT_BEFORE_HEIGHT_ABSOLUTE,
    "-1",
    ErrorCode::AssertBeforeHeightAbsolute
)]
#[case(
    ASSERT_BEFORE_HEIGHT_ABSOLUTE,
    "0",
    ErrorCode::ImpossibleHeightAbsoluteConstraints
)]
#[case(
    ASSERT_BEFORE_HEIGHT_RELATIVE,
    "-1",
    ErrorCode::AssertBeforeHeightRelative
)]
#[case(ASSERT_MY_BIRTH_HEIGHT, "-1", ErrorCode::AssertMyBirthHeightFailed)]
#[case(
    ASSERT_MY_BIRTH_HEIGHT,
    "0x0100000000",
    ErrorCode::AssertMyBirthHeightFailed
)]
#[case(ASSERT_MY_BIRTH_SECONDS, "-1", ErrorCode::AssertMyBirthSecondsFailed)]
#[case(
    ASSERT_MY_BIRTH_SECONDS,
    "0x010000000000000000",
    ErrorCode::AssertMyBirthSecondsFailed
)]
fn test_single_condition_failure(
    #[case] condition: ConditionOpcode,
    #[case] arg: &str,
    #[case] expected_error: ErrorCode,
) {
    let err = cond_test(&format!(
        "((({{h1}} ({{h2}} (123 ((({} ({} )))))",
        condition as u8, arg
    ))
    .unwrap_err()
    .1;

    assert_eq!(err, expected_error);
}

// this test ensures that the ASSERT_BEFORE_ condition codes are not available
// unless the ENABLE_ASSERT_BEFORE flag is set
#[cfg(test)]
#[rstest]
#[case(ASSERT_SECONDS_ABSOLUTE, "104", None)]
#[case(ASSERT_SECONDS_RELATIVE, "101", None)]
#[case(ASSERT_HEIGHT_RELATIVE, "101", None)]
#[case(ASSERT_HEIGHT_ABSOLUTE, "100", None)]
#[case(
    ASSERT_BEFORE_SECONDS_ABSOLUTE,
    "104",
    Some(ErrorCode::InvalidConditionOpcode)
)]
#[case(
    ASSERT_BEFORE_SECONDS_RELATIVE,
    "101",
    Some(ErrorCode::InvalidConditionOpcode)
)]
#[case(
    ASSERT_BEFORE_HEIGHT_RELATIVE,
    "101",
    Some(ErrorCode::InvalidConditionOpcode)
)]
#[case(
    ASSERT_BEFORE_HEIGHT_ABSOLUTE,
    "100",
    Some(ErrorCode::InvalidConditionOpcode)
)]
#[case(RESERVE_FEE, "100", None)]
#[case(ASSERT_MY_AMOUNT, "123", None)]
#[case(
    ASSERT_MY_BIRTH_SECONDS,
    "123",
    Some(ErrorCode::InvalidConditionOpcode)
)]
#[case(ASSERT_MY_BIRTH_HEIGHT, "123", Some(ErrorCode::InvalidConditionOpcode))]
#[case(ASSERT_MY_COIN_ID, "{coin12}", None)]
#[case(ASSERT_MY_PARENT_ID, "{h1}", None)]
#[case(ASSERT_MY_PUZZLEHASH, "{h2}", None)]
#[case(
    ASSERT_CONCURRENT_SPEND,
    "{coin12}",
    Some(ErrorCode::InvalidConditionOpcode)
)]
#[case(
    ASSERT_CONCURRENT_PUZZLE,
    "{coin12}",
    Some(ErrorCode::InvalidConditionOpcode)
)]
fn test_disable_assert_before(
    #[case] condition: ConditionOpcode,
    #[case] arg: &str,
    #[case] expected_error: Option<ErrorCode>,
) {
    // The flag we pass in does not have the ENABLE_ASSERT_BEFORE flag set.
    // Setting the NO_UNKNOWN_CONDS will make those opcodes fail
    let ret = cond_test_flag(
        &format!(
            "((({{h1}} ({{h2}} (123 ((({} ({} )))))",
            condition as u8, arg
        ),
        NO_UNKNOWN_CONDS,
    );

    if let Some(err) = expected_error {
        assert_eq!(ret.unwrap_err().1, err);
    } else {
        let (_, conds) = ret.unwrap();
        assert_eq!(conds.cost, 0);
        assert_eq!(conds.spends.len(), 1);
        let spend = &conds.spends[0];
        assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    }
}

// this test includes multiple instances of the same condition, to ensure we
// aggregate the resulting condition correctly. The values we pass are:
// 100, 503, 90
#[cfg(test)]
#[rstest]
// we use the MAX value
#[case(ASSERT_SECONDS_ABSOLUTE, |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.seconds_absolute, 503))]
#[case(ASSERT_SECONDS_RELATIVE, |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.seconds_relative, 503))]
#[case(ASSERT_HEIGHT_RELATIVE, |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.height_relative, Some(503)))]
#[case(ASSERT_HEIGHT_ABSOLUTE, |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.height_absolute, 503))]
// we use the SUM of the values
#[case(RESERVE_FEE, |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.reserve_fee, 693))]
// we use the MIN value
#[case(ASSERT_BEFORE_SECONDS_ABSOLUTE, |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.before_seconds_absolute, Some(90)))]
#[case(ASSERT_BEFORE_SECONDS_RELATIVE, |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.before_seconds_relative, Some(90)))]
#[case(ASSERT_BEFORE_HEIGHT_RELATIVE, |_: &SpendBundleConditions, s: &Spend| assert_eq!(s.before_height_relative, Some(90)))]
#[case(ASSERT_BEFORE_HEIGHT_ABSOLUTE, |c: &SpendBundleConditions, _: &Spend| assert_eq!(c.before_height_absolute, Some(90)))]
fn test_multiple_conditions(
    #[case] condition: ConditionOpcode,
    #[case] test: impl Fn(&SpendBundleConditions, &Spend),
) {
    let val = condition as u8;
    let (a, conds) = cond_test(&format!(
        "((({{h1}} ({{h2}} (1234 ((({val} (100 ) (({val} (503 ) (({val} (90 )))))"
    ))
    .unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 1234));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);

    test(&conds, &spend);
}

// parse all conditions without an argument. They should all fail
#[cfg(test)]
#[rstest]
#[case(ASSERT_SECONDS_ABSOLUTE)]
#[case(ASSERT_SECONDS_RELATIVE)]
#[case(ASSERT_HEIGHT_RELATIVE)]
#[case(ASSERT_HEIGHT_ABSOLUTE)]
#[case(RESERVE_FEE)]
#[case(CREATE_COIN_ANNOUNCEMENT)]
#[case(ASSERT_COIN_ANNOUNCEMENT)]
#[case(CREATE_PUZZLE_ANNOUNCEMENT)]
#[case(ASSERT_PUZZLE_ANNOUNCEMENT)]
#[case(ASSERT_MY_AMOUNT)]
#[case(ASSERT_MY_BIRTH_SECONDS)]
#[case(ASSERT_MY_BIRTH_HEIGHT)]
#[case(ASSERT_MY_COIN_ID)]
#[case(ASSERT_MY_PARENT_ID)]
#[case(ASSERT_MY_PUZZLEHASH)]
#[case(CREATE_COIN)]
#[case(AGG_SIG_UNSAFE)]
#[case(AGG_SIG_ME)]
#[case(ASSERT_CONCURRENT_SPEND)]
#[case(ASSERT_CONCURRENT_PUZZLE)]
fn test_missing_arg(#[case] condition: ConditionOpcode) {
    // extra args are disallowed in mempool mode
    assert_eq!(
        cond_test_flag(
            &format!("((({{h1}} ({{h2}} (123 ((({} )))))", condition as u8),
            ENABLE_ASSERT_BEFORE
        )
        .unwrap_err()
        .1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_single_height_relative_zero() {
    // ASSERT_HEIGHT_RELATIVE
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((82 (0 )))))").unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);

    assert_eq!(spend.height_relative, Some(0));
}

#[test]
fn test_reserve_fee_exceed_max() {
    // RESERVE_FEE
    // 0xfffffffffffffff0 + 0x10 just exceeds u64::MAX, which is higher than
    // allowed. Note that we need two coins to provide the removals to cover the
    // reserve fee
    // "((({h1} ({h2} (123 (((60 ({msg1} ))) (({h2} ({h2} (123 (((61 ({c11} )))))")
    assert_eq!(
        cond_test("((({h1} ({h2} (0x00ffffffffffffffff (((52 (0x00fffffffffffffff0 ))) (({h2} ({h1} (0x00ffffff (((52 (0x10 )))))")
            .unwrap_err()
            .1,
        ErrorCode::ReserveFeeConditionFailed
    );
}

#[test]
fn test_reserve_fee_insufficient_spends() {
    // RESERVE_FEE
    // We spend a coin with amount 123 but reserve fee 124
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((52 (124 ) ))))")
            .unwrap_err()
            .1,
        ErrorCode::ReserveFeeConditionFailed
    );
}

#[test]
fn test_reserve_fee_insufficient_fee() {
    // RESERVE_FEE
    // We spend a coin with amount 123 and create a coin worth 24 and reserve fee
    // of 100 (which adds up to 124, i.e. not enough fee)
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((52 (100 ) ((51 ({h2} (24 )) )))")
            .unwrap_err()
            .1,
        ErrorCode::ReserveFeeConditionFailed
    );
}

// TOOD: test announcement across coins

#[test]
fn test_coin_announces_consume() {
    // CREATE_COIN_ANNOUNCEMENT
    // ASSERT_COIN_ANNOUNCEMENT
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((60 ({msg1} ) ((61 ({c11} )))))").unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_cross_coin_announces_consume() {
    // CREATE_COIN_ANNOUNCEMENT
    // ASSERT_COIN_ANNOUNCEMENT
    let (a, conds) =
        cond_test("((({h1} ({h2} (123 (((60 ({msg1} ))) (({h2} ({h2} (123 (((61 ({c11} )))))")
            .unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 2);
    assert_eq!(*conds.spends[0].coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(conds.spends[0].puzzle_hash), H2);
    assert_eq!(*conds.spends[1].coin_id, test_coin_id(H2, H2, 123));
    assert_eq!(a.atom(conds.spends[1].puzzle_hash), H2);
}

#[test]
fn test_failing_coin_consume() {
    // ASSERT_COIN_ANNOUNCEMENT
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((61 ({c11} )))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertCoinAnnouncementFailed
    );
}

#[test]
fn test_coin_announce_mismatch() {
    // CREATE_COIN_ANNOUNCEMENT
    // ASSERT_COIN_ANNOUNCEMENT
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((60 ({msg1} ) ((61 ({c12} )))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertCoinAnnouncementFailed
    );
}

#[test]
fn test_puzzle_announces_consume() {
    // CREATE_PUZZLE_ANNOUNCEMENT
    // ASSERT_PUZZLE_ANNOUNCEMENT
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((62 ({msg1} ) ((63 ({p21} )))))").unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_cross_coin_puzzle_announces_consume() {
    // CREATE_PUZZLE_ANNOUNCEMENT
    // ASSERT_PUZZLE_ANNOUNCEMENT
    let (a, conds) =
        cond_test("((({h1} ({h2} (123 (((62 ({msg1} ))) (({h2} ({h2} (123 (((63 ({p21} )))))")
            .unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 2);
    assert_eq!(*conds.spends[0].coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(conds.spends[0].puzzle_hash), H2);
    assert_eq!(*conds.spends[1].coin_id, test_coin_id(H2, H2, 123));
    assert_eq!(a.atom(conds.spends[1].puzzle_hash), H2);
}

#[test]
fn test_failing_puzzle_consume() {
    // ASSERT_PUZZLE_ANNOUNCEMENT
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((63 ({p21} )))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertPuzzleAnnouncementFailed
    );
}

#[test]
fn test_puzzle_announce_mismatch() {
    // CREATE_PUZZLE_ANNOUNCEMENT
    // ASSERT_PUZZLE_ANNOUNCEMENT
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((62 ({msg1} ) ((63 ({p11} )))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertPuzzleAnnouncementFailed
    );
}

#[test]
fn test_single_assert_my_amount_exceed_max() {
    // ASSERT_MY_AMOUNT
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((73 (0x010000000000000000 )))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertMyAmountFailed
    );
}

#[test]
fn test_single_assert_my_amount_overlong() {
    // ASSERT_MY_AMOUNT
    // leading zeroes are disallowed
    assert_eq!(
        cond_test_flag("((({h1} ({h2} (123 (((73 (0x0000007b )))))", 0)
            .unwrap_err()
            .1,
        ErrorCode::AssertMyAmountFailed
    );
}

#[test]
fn test_single_assert_my_amount_overlong_mempool() {
    // ASSERT_MY_AMOUNT
    // leading zeroes are disallowed in mempool mode
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((73 (0x0000007b )))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertMyAmountFailed
    );
}

#[test]
fn test_multiple_assert_my_amount() {
    // ASSERT_MY_AMOUNT
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((73 (123 ) ((73 (123 ) ))))").unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_multiple_failing_assert_my_amount() {
    // ASSERT_MY_AMOUNT
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((73 (123 ) ((73 (122 ) ))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertMyAmountFailed
    );
}

#[test]
fn test_single_failing_assert_my_amount() {
    // ASSERT_MY_AMOUNT
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((73 (124 ) ))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertMyAmountFailed
    );
}

#[test]
fn test_single_assert_my_coin_id_overlong() {
    // ASSERT_MY_COIN_ID
    // leading zeros in the coin amount invalid
    assert_eq!(
        cond_test_flag("((({h1} ({h2} (0x0000007b (((70 ({coin12} )))))", 0)
            .unwrap_err()
            .1,
        ErrorCode::InvalidCoinAmount
    );
}

#[test]
fn test_multiple_assert_my_coin_id() {
    // ASSERT_MY_COIN_ID
    let (a, conds) =
        cond_test("((({h1} ({h2} (123 (((70 ({coin12} ) ((70 ({coin12} ) ))))").unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_single_assert_my_coin_id_mismatch() {
    // ASSERT_MY_COIN_ID
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((70 ({coin11} )))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertMyCoinIdFailed
    );
}

#[test]
fn test_multiple_assert_my_coin_id_mismatch() {
    // ASSERT_MY_COIN_ID
    // ASSERT_MY_AMOUNT
    // the coin-ID check matches the *other* coin, not itself
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((60 (123 ))) (({h1} ({h1} (123 (((70 ({coin12} )))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertMyCoinIdFailed
    );
}

#[test]
fn test_multiple_assert_my_parent_coin_id() {
    // ASSERT_MY_PARENT_ID
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((71 ({h1} ) ((71 ({h1} ) ))))").unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_single_assert_my_parent_coin_id_mismatch() {
    // ASSERT_MY_PARENT_ID
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((71 ({h2} )))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertMyParentIdFailed
    );
}

#[test]
fn test_single_invalid_assert_my_parent_coin_id() {
    // ASSERT_MY_PARENT_ID
    // the parent ID in the condition is 33 bytes long
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((71 ({long} )))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertMyParentIdFailed
    );
}

#[test]
fn test_multiple_assert_my_puzzle_hash() {
    // ASSERT_MY_PUZZLEHASH
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((72 ({h2} ) ((72 ({h2} ) ))))").unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_single_assert_my_puzzle_hash_mismatch() {
    // ASSERT_MY_PUZZLEHASH
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((72 ({h1} )))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertMyPuzzlehashFailed
    );
}

#[test]
fn test_single_invalid_assert_my_puzzle_hash() {
    // ASSERT_MY_PUZZLEHASH
    // the parent ID in the condition is 33 bytes long
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((72 ({long} )))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertMyPuzzlehashFailed
    );
}

#[test]
fn test_single_create_coin() {
    // CREATE_COIN
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((51 ({h2} (42 )))))").unwrap();

    assert_eq!(conds.cost, CREATE_COIN_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 42);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert_eq!(c.puzzle_hash, H2.into());
        assert_eq!(c.amount, 42_u64);
        assert_eq!(c.hint, a.null());
    }
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_create_coin_max_amount() {
    // CREATE_COIN
    let (a, conds) =
        cond_test("((({h1} ({h2} (0x00ffffffffffffffff (((51 ({h2} (0x00ffffffffffffffff )))))")
            .unwrap();

    assert_eq!(conds.cost, CREATE_COIN_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 0xffffffffffffffff);
    assert_eq!(conds.addition_amount, 0xffffffffffffffff);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 0xffffffffffffffff));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert_eq!(c.puzzle_hash, H2.into());
        assert_eq!(c.amount, 0xffffffffffffffff_u64);
        assert_eq!(c.hint, a.null());
    }
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_minting_coin() {
    // CREATE_COIN
    // we spend a coin with value 123 but create a coin with value 124
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((51 ({h2} (124 )))))")
            .unwrap_err()
            .1,
        ErrorCode::MintingCoin
    );
}

#[test]
fn test_create_coin_amount_exceeds_max() {
    // CREATE_COIN
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((51 ({h2} (0x010000000000000000 )))))")
            .unwrap_err()
            .1,
        ErrorCode::AmountExceedsMaximum
    );
}

#[test]
fn test_create_coin_negative_amount() {
    // CREATE_COIN
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((51 ({h2} (-1 )))))")
            .unwrap_err()
            .1,
        ErrorCode::NegativeAmount
    );
}

#[test]
fn test_create_coin_invalid_puzzlehash() {
    // CREATE_COIN
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((51 ({long} (42 )))))")
            .unwrap_err()
            .1,
        ErrorCode::InvalidPuzzleHash
    );
}

#[test]
fn test_create_coin_with_hint() {
    // CREATE_COIN
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((51 ({h2} (42 (({h1}) )))))").unwrap();

    assert_eq!(conds.cost, CREATE_COIN_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 42);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert!(c.puzzle_hash == H2.into());
        assert!(c.amount == 42_u64);
        assert!(a.atom(c.hint) == H1.to_vec());
    }
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_create_coin_extra_arg() {
    // CREATE_COIN
    // extra args are allowed in non-mempool mode
    let (a, conds) =
        cond_test_flag("((({h1} ({h2} (123 (((51 ({h2} (42 (({h1}) (1337 )))))", 0).unwrap();

    assert_eq!(conds.cost, CREATE_COIN_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 42);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert!(c.puzzle_hash == H2.into());
        assert!(c.amount == 42_u64);
        assert!(a.atom(c.hint) == H1.to_vec());
    }
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_create_coin_with_multiple_hints() {
    // CREATE_COIN
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((51 ({h2} (42 (({h1} ({h2}) )))))").unwrap();

    assert_eq!(conds.cost, CREATE_COIN_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 42);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert!(c.puzzle_hash == H2.into());
        assert!(c.amount == 42_u64);
        assert!(a.atom(c.hint) == H1.to_vec());
    }
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_create_coin_with_hint_as_atom() {
    // CREATE_COIN
    // the hint is supposed to be a list
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((51 ({h2} (42 ({h1} )))))").unwrap();

    assert_eq!(conds.cost, CREATE_COIN_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 42);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert_eq!(c.puzzle_hash, H2.into());
        assert_eq!(c.amount, 42_u64);
        assert_eq!(c.hint, a.null());
    }
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_create_coin_with_invalid_hint_as_terminator() {
    // CREATE_COIN
    let (a, conds) = cond_test_flag("((({h1} ({h2} (123 (((51 ({h2} (42 {h1}))))", 0).unwrap();

    assert_eq!(conds.cost, CREATE_COIN_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 42);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert_eq!(c.puzzle_hash, H2.into());
        assert_eq!(c.amount, 42_u64);
        assert_eq!(c.hint, a.null());
    }
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_create_coin_with_invalid_hint_as_terminator_mempool() {
    // CREATE_COIN
    // in mempool mode it's not OK to have an invalid terminator
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((51 ({h2} (42 {h1}))))")
            .unwrap_err()
            .1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_create_coin_with_short_hint() {
    // CREATE_COIN
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((51 ({h2} (42 (({msg1}))))))").unwrap();

    assert_eq!(conds.cost, CREATE_COIN_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 42);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.create_coin.len(), 1);

    for c in &spend.create_coin {
        assert!(c.puzzle_hash == H2.into());
        assert!(c.amount == 42_u64);
        assert!(a.atom(c.hint) == MSG1.to_vec());
    }
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_create_coin_with_long_hint() {
    // CREATE_COIN
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((51 ({h2} (42 ({long})))))").unwrap();

    assert_eq!(conds.cost, CREATE_COIN_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 42);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.create_coin.len(), 1);

    for c in &spend.create_coin {
        assert_eq!(c.puzzle_hash, H2.into());
        assert_eq!(c.amount, 42_u64);
        assert_eq!(c.hint, a.null());
    }
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_create_coin_with_pair_hint() {
    // CREATE_COIN
    // we only pick out the first element
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((51 ({h2} (42 (({h1} {h2} )))))").unwrap();

    assert_eq!(conds.cost, CREATE_COIN_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 42);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.create_coin.len(), 1);

    for c in &spend.create_coin {
        assert_eq!(c.puzzle_hash, H2.into());
        assert_eq!(c.amount, 42_u64);
        assert_eq!(a.atom(c.hint), H1.to_vec());
    }
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_create_coin_with_cons_hint() {
    // CREATE_COIN
    // if the first element is a cons-box, it's not a valid hint
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((51 ({h2} (42 ((({h1} {h2}) )))))").unwrap();

    assert_eq!(conds.cost, CREATE_COIN_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 42);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert_eq!(c.puzzle_hash, H2.into());
        assert_eq!(c.amount, 42_u64);
        assert_eq!(c.hint, a.null());
    }
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_multiple_create_coin() {
    // CREATE_COIN
    let (a, conds) =
        cond_test("((({h1} ({h2} (123 (((51 ({h2} (42 ) ((51 ({h2} (43 ) ))))").unwrap();

    assert_eq!(conds.cost, CREATE_COIN_COST * 2);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 42 + 43);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.create_coin.len(), 2);

    assert!(spend.create_coin.contains(&NewCoin {
        puzzle_hash: H2.into(),
        amount: 42_u64,
        hint: a.null()
    }));
    assert!(spend.create_coin.contains(&NewCoin {
        puzzle_hash: H2.into(),
        amount: 43_u64,
        hint: a.null()
    }));
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_create_coin_exceed_cost() {
    // CREATE_COIN
    // ensure that we terminate parsing conditions once they exceed the max cost
    assert_eq!(
        cond_test_cb(
            "((({h1} ({h2} (123 ({} )))",
            0,
            Some(|a: &mut Allocator| -> NodePtr {
                let mut rest: NodePtr = a.null();

                for i in 0..6500 {
                    // this builds one CREATE_COIN condition
                    // borrow-rules prevent this from being succint
                    let coin = a.null();
                    let val = a.new_atom(&u64_to_bytes(i)).unwrap();
                    let coin = a.new_pair(val, coin).unwrap();
                    let val = a.new_atom(H2).unwrap();
                    let coin = a.new_pair(val, coin).unwrap();
                    let val = a.new_atom(&u64_to_bytes(CREATE_COIN as u64)).unwrap();
                    let coin = a.new_pair(val, coin).unwrap();

                    // add the CREATE_COIN condition to the list (called rest)
                    rest = a.new_pair(coin, rest).unwrap();
                }
                rest
            })
        )
        .unwrap_err()
        .1,
        ErrorCode::CostExceeded
    );
}

#[test]
fn test_duplicate_create_coin() {
    // CREATE_COIN
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((51 ({h2} (42 ) ((51 ({h2} (42 ) ))))")
            .unwrap_err()
            .1,
        ErrorCode::DuplicateOutput
    );
}

#[test]
fn test_single_agg_sig_me() {
    // AGG_SIG_ME
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((50 ({pubkey} ({msg1} )))))").unwrap();

    assert_eq!(conds.cost, AGG_SIG_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.agg_sig_me.len(), 1);
    for c in &spend.agg_sig_me {
        assert_eq!(a.atom(c.0), PUBKEY);
        assert_eq!(a.atom(c.1), MSG1);
    }
    assert_eq!(spend.flags, 0);
}

#[test]
fn test_duplicate_agg_sig_me() {
    // AGG_SIG_ME
    // we cannot deduplicate AGG_SIG conditions. Their signatures will be
    // aggregated, and so must all copies of the public keys
    let (a, conds) =
        cond_test("((({h1} ({h2} (123 (((50 ({pubkey} ({msg1} ) ((50 ({pubkey} ({msg1} ) ))))")
            .unwrap();

    assert_eq!(conds.cost, AGG_SIG_COST * 2);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.agg_sig_me.len(), 2);
    for c in &spend.agg_sig_me {
        assert_eq!(a.atom(c.0), PUBKEY);
        assert_eq!(a.atom(c.1), MSG1);
    }
    assert_eq!(spend.flags, 0);
}

#[test]
fn test_agg_sig_me_invalid_pubkey() {
    // AGG_SIG_ME
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((50 ({h2} ({msg1} )))))")
            .unwrap_err()
            .1,
        ErrorCode::InvalidPubkey
    );
}

#[test]
fn test_agg_sig_me_invalid_msg() {
    // AGG_SIG_ME
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((50 ({pubkey} ({longmsg} )))))")
            .unwrap_err()
            .1,
        ErrorCode::InvalidMessage
    );
}

#[test]
fn test_agg_sig_me_exceed_cost() {
    // AGG_SIG_ME
    // ensure that we terminate parsing conditions once they exceed the max cost
    assert_eq!(
        cond_test_cb(
            "((({h1} ({h2} (123 ({} )))",
            0,
            Some(|a: &mut Allocator| -> NodePtr {
                let mut rest: NodePtr = a.null();

                for _i in 0..9167 {
                    // this builds one AGG_SIG_ME condition
                    // borrow-rules prevent this from being succint
                    let aggsig = a.null();
                    let val = a.new_atom(MSG1).unwrap();
                    let aggsig = a.new_pair(val, aggsig).unwrap();
                    let val = a.new_atom(PUBKEY).unwrap();
                    let aggsig = a.new_pair(val, aggsig).unwrap();
                    let val = a.new_atom(&u64_to_bytes(AGG_SIG_ME as u64)).unwrap();
                    let aggsig = a.new_pair(val, aggsig).unwrap();

                    // add the AGG_SIG_ME condition to the list (called rest)
                    rest = a.new_pair(aggsig, rest).unwrap();
                }
                rest
            })
        )
        .unwrap_err()
        .1,
        ErrorCode::CostExceeded
    );
}

#[test]
fn test_single_agg_sig_unsafe() {
    // AGG_SIG_UNSAFE
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((49 ({pubkey} ({msg1} )))))").unwrap();

    assert_eq!(conds.cost, AGG_SIG_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(conds.agg_sig_unsafe.len(), 1);
    for (pk, msg) in &conds.agg_sig_unsafe {
        assert_eq!(a.atom(*pk), PUBKEY);
        assert_eq!(a.atom(*msg), MSG1);
    }
    assert_eq!(spend.flags, 0);
}

#[test]
fn test_agg_sig_unsafe_extra_arg() {
    // AGG_SIG_UNSAFE
    // extra args are disallowed in non-mempool mode
    assert_eq!(
        cond_test_flag("((({h1} ({h2} (123 (((49 ({pubkey} ({msg1} (456 )))))", 0)
            .unwrap_err()
            .1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_agg_sig_me_extra_arg() {
    // AGG_SIG_ME
    // extra args are disallowed in non-mempool mode
    assert_eq!(
        cond_test_flag("((({h1} ({h2} (123 (((50 ({pubkey} ({msg1} (456 )))))", 0)
            .unwrap_err()
            .1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_agg_sig_unsafe_invalid_terminator() {
    // AGG_SIG_UNSAFE
    // in non-mempool mode, even an invalid terminator is allowed
    let (a, conds) =
        cond_test_flag("((({h1} ({h2} (123 (((49 ({pubkey} ({msg1} 456 ))))", 0).unwrap();

    assert_eq!(conds.cost, AGG_SIG_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(conds.agg_sig_unsafe.len(), 1);
    for (pk, msg) in &conds.agg_sig_unsafe {
        assert_eq!(a.atom(*pk), PUBKEY);
        assert_eq!(a.atom(*msg), MSG1);
    }
    assert_eq!(spend.flags, 0);
}

#[test]
fn test_agg_sig_unsafe_invalid_terminator_mempool() {
    // AGG_SIG_UNSAFE
    assert_eq!(
        cond_test_flag(
            "((({h1} ({h2} (123 (((49 ({pubkey} ({msg1} 456 ))))",
            COND_ARGS_NIL
        )
        .unwrap_err()
        .1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_agg_sig_me_invalid_terminator() {
    // AGG_SIG_ME
    // this has an invalid list terminator of the argument list. This is OK
    // according to the original consensus rules
    let (a, conds) =
        cond_test_flag("((({h1} ({h2} (123 (((50 ({pubkey} ({msg1} 456 ))))", 0).unwrap();

    assert_eq!(conds.cost, AGG_SIG_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.agg_sig_me.len(), 1);
    for (pk, msg) in &conds.agg_sig_unsafe {
        assert_eq!(a.atom(*pk), PUBKEY);
        assert_eq!(a.atom(*msg), MSG1);
    }
    assert_eq!(spend.flags, 0);
}

#[test]
fn test_agg_sig_me_invalid_terminator_mempool() {
    // AGG_SIG_ME
    // this has an invalid list terminator of the argument list. This is NOT OK
    // according to the stricter rules
    assert_eq!(
        cond_test_flag(
            "((({h1} ({h2} (123 (((50 ({pubkey} ({msg1} 456 ))))",
            COND_ARGS_NIL
        )
        .unwrap_err()
        .1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_duplicate_agg_sig_unsafe() {
    // AGG_SIG_UNSAFE
    // these conditions may not be deduplicated
    let (a, conds) =
        cond_test("((({h1} ({h2} (123 (((49 ({pubkey} ({msg1} ) ((49 ({pubkey} ({msg1} ) ))))")
            .unwrap();

    assert_eq!(conds.cost, AGG_SIG_COST * 2);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(conds.agg_sig_unsafe.len(), 2);
    for (pk, msg) in &conds.agg_sig_unsafe {
        assert_eq!(a.atom(*pk), PUBKEY);
        assert_eq!(a.atom(*msg), MSG1);
    }
    assert_eq!(spend.flags, 0);
}

#[test]
fn test_agg_sig_unsafe_invalid_pubkey() {
    // AGG_SIG_UNSAFE
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((49 ({h2} ({msg1} )))))")
            .unwrap_err()
            .1,
        ErrorCode::InvalidPubkey
    );
}

#[test]
fn test_agg_sig_unsafe_invalid_msg() {
    // AGG_SIG_UNSAFE
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((49 ({pubkey} ({longmsg} )))))")
            .unwrap_err()
            .1,
        ErrorCode::InvalidMessage
    );
}

#[test]
fn test_agg_sig_unsafe_exceed_cost() {
    // AGG_SIG_UNSAFE
    // ensure that we terminate parsing conditions once they exceed the max cost
    assert_eq!(
        cond_test_cb(
            "((({h1} ({h2} (123 ({} )))",
            0,
            Some(|a: &mut Allocator| -> NodePtr {
                let mut rest: NodePtr = a.null();

                for _i in 0..9167 {
                    // this builds one AGG_SIG_UNSAFE condition
                    // borrow-rules prevent this from being succint
                    let aggsig = a.null();
                    let val = a.new_atom(MSG1).unwrap();
                    let aggsig = a.new_pair(val, aggsig).unwrap();
                    let val = a.new_atom(PUBKEY).unwrap();
                    let aggsig = a.new_pair(val, aggsig).unwrap();
                    let val = a.new_atom(&u64_to_bytes(AGG_SIG_UNSAFE as u64)).unwrap();
                    let aggsig = a.new_pair(val, aggsig).unwrap();

                    // add the AGG_SIG_UNSAFE condition to the list (called rest)
                    rest = a.new_pair(aggsig, rest).unwrap();
                }
                rest
            })
        )
        .unwrap_err()
        .1,
        ErrorCode::CostExceeded
    );
}

#[test]
fn test_spend_amount_exceeds_max() {
    // the coin we're trying to spend has an amount that exceeds maximum
    assert_eq!(
        cond_test("((({h1} ({h2} (0x010000000000000000 ())))")
            .unwrap_err()
            .1,
        ErrorCode::InvalidCoinAmount
    );
}

#[test]
fn test_single_spend_negative_amount() {
    // the coin we're trying to spend has a negative amount (i.e. it's invalid)
    assert_eq!(
        cond_test("((({h1} ({h2} (-123 ())))").unwrap_err().1,
        ErrorCode::InvalidCoinAmount
    );
}

#[test]
fn test_single_spend_invalid_puzle_hash() {
    // the puzzle hash in the spend is 33 bytes
    assert_eq!(
        cond_test("((({h1} ({long} (123 ())))").unwrap_err().1,
        ErrorCode::InvalidPuzzleHash
    );
}

#[test]
fn test_single_spend_invalid_parent_id() {
    // the parent coin ID is 33 bytes long
    assert_eq!(
        cond_test("((({long} ({h2} (123 ())))").unwrap_err().1,
        ErrorCode::InvalidParentId
    );
}

#[test]
fn test_double_spend() {
    // we spend the same coin twice
    assert_eq!(
        cond_test("((({h1} ({h2} (123 ()) (({h1} ({h2} (123 ())))")
            .unwrap_err()
            .1,
        ErrorCode::DoubleSpend
    );
}

#[test]
fn test_always_true() {
    // ALWAYS_TRUE
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((1 )))))").unwrap();

    // just make sure there are no constraints
    assert_eq!(conds.agg_sig_unsafe.len(), 0);
    assert_eq!(conds.reserve_fee, 0);
    assert_eq!(conds.height_absolute, 0);
    assert_eq!(conds.seconds_absolute, 0);
    assert_eq!(conds.cost, 0);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);

    // there is one spend
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.agg_sig_me.len(), 0);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_always_true_with_arg() {
    // ALWAYS_TRUE, but with one unknown argument
    // unknown arguments are expected and always allowed
    let (a, conds) = cond_test("((({h1} ({h2} (123 (((1 ( 42 )))))").unwrap();

    // just make sure there are no constraints
    assert_eq!(conds.agg_sig_unsafe.len(), 0);
    assert_eq!(conds.reserve_fee, 0);
    assert_eq!(conds.height_absolute, 0);
    assert_eq!(conds.seconds_absolute, 0);
    assert_eq!(conds.cost, 0);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);

    // there is one spend
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.agg_sig_me.len(), 0);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_concurrent_spend() {
    // ASSERT_CONCURRENT_SPEND
    // this spends the coin (h1, h2, 123)
    // and (h2, h2, 123).

    // three cases are tested:
    // 1. the first spend asserts that the second happens
    // 2. the second asserts that the first happens.
    // 3. the second asserts its own coin ID
    // the result is the same in all cases, and all are expected to pass

    let test_cases = [
        "(\
            (({h1} ({h2} (123 (((64 ({coin22} )))\
            (({h2} ({h2} (123 ())\
            ))",
        "(\
            (({h1} ({h2} (123 ())\
            (({h2} ({h2} (123 (((64 ({coin12} )))\
            ))",
        "(\
            (({h1} ({h2} (123 ())\
            (({h2} ({h2} (123 (((64 ({coin22} )))\
            ))",
    ];

    for test in test_cases {
        let (a, conds) = cond_test(test).unwrap();

        // just make sure there are no constraints
        assert_eq!(conds.agg_sig_unsafe.len(), 0);
        assert_eq!(conds.reserve_fee, 0);
        assert_eq!(conds.height_absolute, 0);
        assert_eq!(conds.seconds_absolute, 0);
        assert_eq!(conds.cost, 0);
        assert_eq!(conds.removal_amount, 246);
        assert_eq!(conds.addition_amount, 0);

        // there are two spends
        assert_eq!(conds.spends.len(), 2);
        let spend = &conds.spends[0];
        assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
        assert_eq!(a.atom(spend.puzzle_hash), H2);
        assert_eq!(spend.agg_sig_me.len(), 0);
        assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);

        let spend = &conds.spends[1];
        assert_eq!(*spend.coin_id, test_coin_id(H2, H2, 123));
        assert_eq!(a.atom(spend.puzzle_hash), H2);
        assert_eq!(spend.agg_sig_me.len(), 0);
        assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
    }
}

#[test]
fn test_concurrent_spend_fail() {
    // ASSERT_CONCURRENT_SPEND
    // this spends the coin (h1, h2, 123)
    // and (h2, h2, 123).

    // this test ensures that asserting a coin ID that's not being spent causes
    // a failure. There are two cases, where each of the two spends make the
    // invalid assertion

    let test_cases = [
        "(\
            (({h1} ({h2} (123 (((64 ({coin21} )))\
            (({h2} ({h2} (123 ())\
            ))",
        "(\
            (({h1} ({h2} (123 ())\
            (({h2} ({h2} (123 (((64 ({coin21} )))\
            ))",
        // msg1 has an invalid length for a sha256 hash
        "(\
            (({h1} ({h2} (123 ())\
            (({h2} ({h2} (123 (((64 ({msg1} )))\
            ))",
        // in this case we *create* coin ((coin12 h2 42))
        // and we assert it being spent (which should fail)
        // i.e. make sure we don't "cross the beams" on created coins and spent
        // coins
        "(\
            (({h1} ({h2} (123 (((51 ({h2} (42 )))\
            (({h2} ({h2} (123 (((64 ({coin12_h2_42} )))\
            ))",
    ];

    for test in test_cases {
        assert_eq!(
            cond_test(test).unwrap_err().1,
            ErrorCode::AssertConcurrentSpendFailed
        );
    }
}

#[test]
fn test_assert_concurrent_spend_self() {
    // ASSERT_CONCURRENT_SPEND
    // asserting ones own coin ID is always true

    let (a, conds) = cond_test(
        "(\
        (({h1} ({h2} (123 (((64 ({coin12} )))\
        ))",
    )
    .unwrap();

    // just make sure there are no constraints
    assert_eq!(conds.agg_sig_unsafe.len(), 0);
    assert_eq!(conds.reserve_fee, 0);
    assert_eq!(conds.height_absolute, 0);
    assert_eq!(conds.seconds_absolute, 0);
    assert_eq!(conds.cost, 0);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);

    // there are two spends
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.agg_sig_me.len(), 0);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_concurrent_puzzle() {
    // ASSERT_CONCURRENT_PUZZLE
    // this spends the coin (h1, h2, 123)
    // and (h2, h2, 123).

    // three cases are tested:
    // 1. the first spend asserts second's puzzle hash
    // 2. the second asserts the first's puzzle hash
    // 3. the second asserts its own puzzle hash
    // the result is the same in all cases, and all are expected to pass

    let test_cases = [
        "(\
            (({h1} ({h1} (123 (((65 ({h2} )))\
            (({h2} ({h2} (123 ())\
            ))",
        "(\
            (({h1} ({h1} (123 ())\
            (({h2} ({h2} (123 (((65 ({h1} )))\
            ))",
        "(\
            (({h1} ({h1} (123 ())\
            (({h2} ({h2} (123 (((65 ({h2} )))\
            ))",
    ];

    for test in test_cases {
        let (a, conds) = cond_test(test).unwrap();

        // just make sure there are no constraints
        assert_eq!(conds.agg_sig_unsafe.len(), 0);
        assert_eq!(conds.reserve_fee, 0);
        assert_eq!(conds.height_absolute, 0);
        assert_eq!(conds.seconds_absolute, 0);
        assert_eq!(conds.cost, 0);
        assert_eq!(conds.removal_amount, 246);
        assert_eq!(conds.addition_amount, 0);

        // there are two spends
        assert_eq!(conds.spends.len(), 2);
        let spend = &conds.spends[0];
        assert_eq!(*spend.coin_id, test_coin_id(H1, H1, 123));
        assert_eq!(a.atom(spend.puzzle_hash), H1);
        assert_eq!(spend.agg_sig_me.len(), 0);
        assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);

        let spend = &conds.spends[1];
        assert_eq!(*spend.coin_id, test_coin_id(H2, H2, 123));
        assert_eq!(a.atom(spend.puzzle_hash), H2);
        assert_eq!(spend.agg_sig_me.len(), 0);
        assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
    }
}

#[test]
fn test_concurrent_puzzle_fail() {
    // ASSERT_CONCURRENT_PUZZLE
    // this spends the coin (h1, h2, 123)
    // and (h2, h2, 123).

    // this test ensures that asserting a puzzle hash that's not being spent
    // causes a failure.

    let test_cases = [
        "(\
            (({h1} ({h2} (123 (((65 ({h1} )))\
            (({h2} ({h2} (123 ())\
            ))",
        "(\
            (({h1} ({h2} (123 ())\
            (({h2} ({h2} (123 (((65 ({h1} )))\
            ))",
        // msg1 has an invalid length for a sha256 hash
        "(\
            (({h1} ({h2} (123 ())\
            (({h2} ({h2} (123 (((65 ({msg1} )))\
            ))",
        // in this case we *create* coin ((coin12 h2 42))
        // i.e. paid to puzzle hash h2. And we assert the puzzle hash h2 is
        // being spent. This should not pass, i.e. make sure we don't "cross the
        // beams" on created coins and spent puzzles
        "(\
            (({h1} ({h1} (123 (((51 ({h2} (42 )))\
            (({h2} ({h1} (123 (((65 ({h2} )))\
            ))",
    ];

    for test in test_cases {
        assert_eq!(
            cond_test(test).unwrap_err().1,
            ErrorCode::AssertConcurrentPuzzleFailed
        );
    }
}

#[test]
fn test_assert_concurrent_puzzle_self() {
    // ASSERT_CONCURRENT_PUZZLE
    // asserting ones own puzzle hash is always true

    let (a, conds) = cond_test(
        "(\
        (({h1} ({h2} (123 (((65 ({h2} )))\
        ))",
    )
    .unwrap();

    // just make sure there are no constraints
    assert_eq!(conds.agg_sig_unsafe.len(), 0);
    assert_eq!(conds.reserve_fee, 0);
    assert_eq!(conds.height_absolute, 0);
    assert_eq!(conds.seconds_absolute, 0);
    assert_eq!(conds.cost, 0);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);

    // there are two spends
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.agg_sig_me.len(), 0);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

// the relative constraints clash because they are on the same coin spend
#[cfg(test)]
#[rstest]
#[case(
    ASSERT_SECONDS_ABSOLUTE,
    100,
    ASSERT_BEFORE_SECONDS_ABSOLUTE,
    100,
    Some(ErrorCode::ImpossibleSecondsAbsoluteConstraints)
)]
#[case(ASSERT_SECONDS_ABSOLUTE, 99, ASSERT_BEFORE_SECONDS_ABSOLUTE, 100, None)]
#[case(
    ASSERT_HEIGHT_ABSOLUTE,
    100,
    ASSERT_BEFORE_HEIGHT_ABSOLUTE,
    100,
    Some(ErrorCode::ImpossibleHeightAbsoluteConstraints)
)]
#[case(ASSERT_HEIGHT_ABSOLUTE, 99, ASSERT_BEFORE_HEIGHT_ABSOLUTE, 100, None)]
#[case(
    ASSERT_SECONDS_RELATIVE,
    100,
    ASSERT_BEFORE_SECONDS_RELATIVE,
    100,
    Some(ErrorCode::ImpossibleSecondsRelativeConstraints)
)]
#[case(ASSERT_SECONDS_RELATIVE, 99, ASSERT_BEFORE_SECONDS_RELATIVE, 100, None)]
#[case(
    ASSERT_HEIGHT_RELATIVE,
    100,
    ASSERT_BEFORE_HEIGHT_RELATIVE,
    100,
    Some(ErrorCode::ImpossibleHeightRelativeConstraints)
)]
#[case(ASSERT_HEIGHT_RELATIVE, 99, ASSERT_BEFORE_HEIGHT_RELATIVE, 100, None)]
// order shouldn't matter
#[case(
    ASSERT_BEFORE_SECONDS_ABSOLUTE,
    100,
    ASSERT_SECONDS_ABSOLUTE,
    100,
    Some(ErrorCode::ImpossibleSecondsAbsoluteConstraints)
)]
#[case(ASSERT_BEFORE_SECONDS_ABSOLUTE, 100, ASSERT_SECONDS_ABSOLUTE, 99, None)]
#[case(
    ASSERT_BEFORE_HEIGHT_ABSOLUTE,
    100,
    ASSERT_HEIGHT_ABSOLUTE,
    100,
    Some(ErrorCode::ImpossibleHeightAbsoluteConstraints)
)]
#[case(ASSERT_BEFORE_HEIGHT_ABSOLUTE, 100, ASSERT_HEIGHT_ABSOLUTE, 99, None)]
#[case(
    ASSERT_BEFORE_SECONDS_RELATIVE,
    100,
    ASSERT_SECONDS_RELATIVE,
    100,
    Some(ErrorCode::ImpossibleSecondsRelativeConstraints)
)]
#[case(ASSERT_BEFORE_SECONDS_RELATIVE, 100, ASSERT_SECONDS_RELATIVE, 99, None)]
#[case(
    ASSERT_BEFORE_HEIGHT_RELATIVE,
    100,
    ASSERT_HEIGHT_RELATIVE,
    100,
    Some(ErrorCode::ImpossibleHeightRelativeConstraints)
)]
#[case(ASSERT_BEFORE_HEIGHT_RELATIVE, 100, ASSERT_HEIGHT_RELATIVE, 99, None)]
fn test_impossible_constraints_single_spend(
    #[case] cond1: ConditionOpcode,
    #[case] value1: u64,
    #[case] cond2: ConditionOpcode,
    #[case] value2: u64,
    #[case] expected_err: Option<ErrorCode>,
) {
    let test: &str = &format!(
        "(\
       (({{h1}} ({{h1}} (123 (\
           (({} ({} ) \
           (({} ({} ) \
           ))\
       ))",
        cond1 as u8, value1, cond2 as u8, value2
    );
    if let Some(e) = expected_err {
        assert_eq!(cond_test(test).unwrap_err().1, e);
    } else {
        // we don't expect any error
        let (a, conds) = cond_test(test).unwrap();

        // just make sure there are no constraints
        assert_eq!(conds.agg_sig_unsafe.len(), 0);
        assert_eq!(conds.reserve_fee, 0);
        assert_eq!(conds.cost, 0);
        assert_eq!(conds.removal_amount, 123);
        assert_eq!(conds.addition_amount, 0);

        assert_eq!(conds.spends.len(), 1);
        let spend = &conds.spends[0];
        assert_eq!(*spend.coin_id, test_coin_id(H1, H1, 123));
        assert_eq!(a.atom(spend.puzzle_hash), H1);
        assert_eq!(spend.agg_sig_me.len(), 0);
        assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
    }
}

// the relative constraints do not clash because they are on separate coin
// spends. We don't know those coins' confirm block height nor timestamps,
// so we can't infer any conflicts
#[cfg(test)]
#[rstest]
#[case(
    ASSERT_SECONDS_ABSOLUTE,
    100,
    ASSERT_BEFORE_SECONDS_ABSOLUTE,
    100,
    Some(ErrorCode::ImpossibleSecondsAbsoluteConstraints)
)]
#[case(ASSERT_SECONDS_ABSOLUTE, 99, ASSERT_BEFORE_SECONDS_ABSOLUTE, 100, None)]
#[case(
    ASSERT_HEIGHT_ABSOLUTE,
    100,
    ASSERT_BEFORE_HEIGHT_ABSOLUTE,
    100,
    Some(ErrorCode::ImpossibleHeightAbsoluteConstraints)
)]
#[case(ASSERT_HEIGHT_ABSOLUTE, 99, ASSERT_BEFORE_HEIGHT_ABSOLUTE, 100, None)]
#[case(
    ASSERT_SECONDS_RELATIVE,
    100,
    ASSERT_BEFORE_SECONDS_RELATIVE,
    100,
    None
)]
#[case(ASSERT_SECONDS_RELATIVE, 99, ASSERT_BEFORE_SECONDS_RELATIVE, 100, None)]
#[case(ASSERT_HEIGHT_RELATIVE, 100, ASSERT_BEFORE_HEIGHT_RELATIVE, 100, None)]
#[case(ASSERT_HEIGHT_RELATIVE, 99, ASSERT_BEFORE_HEIGHT_RELATIVE, 100, None)]
// order shouldn't matter
#[case(
    ASSERT_BEFORE_SECONDS_ABSOLUTE,
    100,
    ASSERT_SECONDS_ABSOLUTE,
    100,
    Some(ErrorCode::ImpossibleSecondsAbsoluteConstraints)
)]
#[case(ASSERT_BEFORE_SECONDS_ABSOLUTE, 100, ASSERT_SECONDS_ABSOLUTE, 99, None)]
#[case(
    ASSERT_BEFORE_HEIGHT_ABSOLUTE,
    100,
    ASSERT_HEIGHT_ABSOLUTE,
    100,
    Some(ErrorCode::ImpossibleHeightAbsoluteConstraints)
)]
#[case(ASSERT_BEFORE_HEIGHT_ABSOLUTE, 100, ASSERT_HEIGHT_ABSOLUTE, 99, None)]
#[case(
    ASSERT_BEFORE_SECONDS_RELATIVE,
    100,
    ASSERT_SECONDS_RELATIVE,
    100,
    None
)]
#[case(ASSERT_BEFORE_SECONDS_RELATIVE, 100, ASSERT_SECONDS_RELATIVE, 99, None)]
#[case(ASSERT_BEFORE_HEIGHT_RELATIVE, 100, ASSERT_HEIGHT_RELATIVE, 100, None)]
#[case(ASSERT_BEFORE_HEIGHT_RELATIVE, 100, ASSERT_HEIGHT_RELATIVE, 99, None)]
fn test_impossible_constraints_separate_spends(
    #[case] cond1: ConditionOpcode,
    #[case] value1: u64,
    #[case] cond2: ConditionOpcode,
    #[case] value2: u64,
    #[case] expected_err: Option<ErrorCode>,
) {
    let test: &str = &format!(
        "(\
       (({{h1}} ({{h1}} (123 (\
           (({} ({} ) \
           ))\
       (({{h1}} ({{h2}} (123 (\
           (({} ({} ) \
           ))\
       ))",
        cond1 as u8, value1, cond2 as u8, value2
    );
    if let Some(e) = expected_err {
        assert_eq!(cond_test(test).unwrap_err().1, e);
    } else {
        // we don't expect any error
        let (a, conds) = cond_test(test).unwrap();

        // just make sure there are no constraints
        assert_eq!(conds.agg_sig_unsafe.len(), 0);
        assert_eq!(conds.reserve_fee, 0);
        assert_eq!(conds.cost, 0);
        assert_eq!(conds.removal_amount, 246);
        assert_eq!(conds.addition_amount, 0);

        assert_eq!(conds.spends.len(), 2);
        let spend = &conds.spends[0];
        assert_eq!(*spend.coin_id, test_coin_id(H1, H1, 123));
        assert_eq!(a.atom(spend.puzzle_hash), H1);
        assert_eq!(spend.agg_sig_me.len(), 0);
        assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);

        let spend = &conds.spends[1];
        assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
        assert_eq!(a.atom(spend.puzzle_hash), H2);
        assert_eq!(spend.agg_sig_me.len(), 0);
        assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
    }
}

#[cfg(test)]
#[rstest]
#[case(ASSERT_MY_BIRTH_HEIGHT, ErrorCode::AssertMyBirthHeightFailed)]
#[case(ASSERT_MY_BIRTH_SECONDS, ErrorCode::AssertMyBirthSecondsFailed)]
fn test_conflicting_my_birth_assertions(
    #[case] condition: ConditionOpcode,
    #[case] expected: ErrorCode,
) {
    let val = condition as u8;
    assert_eq!(
        cond_test(&format!(
            "((({{h1}} ({{h2}} (1234 ((({val} (100 ) (({val} (503 ) (({val} (90 )))))"
        ))
        .unwrap_err()
        .1,
        expected
    );
}

#[cfg(test)]
#[rstest]
#[case(ASSERT_MY_BIRTH_HEIGHT, |s: &Spend| assert_eq!(s.birth_height, Some(100)))]
#[case(ASSERT_MY_BIRTH_SECONDS, |s: &Spend| assert_eq!(s.birth_seconds, Some(100)))]
fn test_multiple_my_birth_assertions(
    #[case] condition: ConditionOpcode,
    #[case] test: impl Fn(&Spend),
) {
    let val = condition as u8;
    let (a, conds) = cond_test(&format!(
        "((({{h1}} ({{h2}} (1234 ((({val} (100 ) (({val} (100 ) (({val} (100 )))))"
    ))
    .unwrap();

    assert_eq!(conds.cost, 0);
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 1234));
    assert_eq!(a.atom(spend.puzzle_hash), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);

    test(spend);
}
