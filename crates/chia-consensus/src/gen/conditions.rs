use super::coin_id::compute_coin_id;
use super::condition_sanitizers::{
    parse_amount, sanitize_announce_msg, sanitize_hash, sanitize_message_mode,
};
use super::opcodes::{
    compute_unknown_condition_cost, parse_opcode, ConditionOpcode, AGG_SIG_AMOUNT, AGG_SIG_COST,
    AGG_SIG_ME, AGG_SIG_PARENT, AGG_SIG_PARENT_AMOUNT, AGG_SIG_PARENT_PUZZLE, AGG_SIG_PUZZLE,
    AGG_SIG_PUZZLE_AMOUNT, AGG_SIG_UNSAFE, ASSERT_BEFORE_HEIGHT_ABSOLUTE,
    ASSERT_BEFORE_HEIGHT_RELATIVE, ASSERT_BEFORE_SECONDS_ABSOLUTE, ASSERT_BEFORE_SECONDS_RELATIVE,
    ASSERT_COIN_ANNOUNCEMENT, ASSERT_CONCURRENT_PUZZLE, ASSERT_CONCURRENT_SPEND, ASSERT_EPHEMERAL,
    ASSERT_HEIGHT_ABSOLUTE, ASSERT_HEIGHT_RELATIVE, ASSERT_MY_AMOUNT, ASSERT_MY_BIRTH_HEIGHT,
    ASSERT_MY_BIRTH_SECONDS, ASSERT_MY_COIN_ID, ASSERT_MY_PARENT_ID, ASSERT_MY_PUZZLEHASH,
    ASSERT_PUZZLE_ANNOUNCEMENT, ASSERT_SECONDS_ABSOLUTE, ASSERT_SECONDS_RELATIVE, CREATE_COIN,
    CREATE_COIN_ANNOUNCEMENT, CREATE_COIN_COST, CREATE_PUZZLE_ANNOUNCEMENT, RECEIVE_MESSAGE,
    REMARK, RESERVE_FEE, SEND_MESSAGE, SOFTFORK,
};
use super::sanitize_int::{sanitize_uint, SanitizedUint};
use super::validation_error::{first, next, rest, ErrorCode, ValidationErr};
use crate::consensus_constants::ConsensusConstants;
use crate::gen::flags::{DONT_VALIDATE_SIGNATURE, NO_UNKNOWN_CONDS, STRICT_ARGS_COUNT};
use crate::gen::make_aggsig_final_message::u64_to_bytes;
use crate::gen::messages::{Message, SpendId};
use crate::gen::spend_visitor::SpendVisitor;
use crate::gen::validation_error::check_nil;
use chia_bls::{aggregate_verify, BlsCache, PublicKey, Signature};
use chia_protocol::{Bytes, Bytes32};
use chia_sha2::Sha256;
use clvmr::allocator::{Allocator, NodePtr, SExp};
use clvmr::cost::Cost;
use std::cmp::{max, min};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

// spend flags

// a spend is eligible for deduplication if it does not have any AGG_SIG_ME
// nor AGG_SIG_UNSAFE
pub const ELIGIBLE_FOR_DEDUP: u32 = 1;

// If the spend bundle contained *any* relative seconds or height condition, this flag is set
pub const HAS_RELATIVE_CONDITION: u32 = 2;

// If the CoinSpend is eligible for fast-forward, this flag is set. A spend is
// eligible if:
// 1. the input coin amount is odd
// 2. There are no AGG_SIG_ME, AGG_SIG_PARENT, AGG_SIG_PARENT_* conditions
// 3. No ASSERT_MY_COIN_ID condition, no more than one ASSERT_MY_PARENT_ID condition
//    (as the second condition)
// 4. it has an output coin with the same puzzle hash as the spend itself
pub const ELIGIBLE_FOR_FF: u32 = 4;

pub struct EmptyVisitor {}

impl SpendVisitor for EmptyVisitor {
    fn new_spend(_spend: &mut SpendConditions) -> Self {
        Self {}
    }
    fn condition(&mut self, _spend: &mut SpendConditions, _c: &Condition) {}
    fn post_spend(&mut self, _a: &Allocator, _spend: &mut SpendConditions) {}
}

pub struct MempoolVisitor {
    condition_counter: i32,
}

impl SpendVisitor for MempoolVisitor {
    fn new_spend(spend: &mut SpendConditions) -> Self {
        // assume it's eligibe. We'll clear this flag if it isn't
        let mut spend_flags = ELIGIBLE_FOR_DEDUP;

        // spend eligible for fast-forward must be singletons, which use odd amounts
        if (spend.coin_amount & 1) == 1 {
            spend_flags |= ELIGIBLE_FOR_FF;
        }
        spend.flags |= spend_flags;

        Self {
            condition_counter: 0,
        }
    }

    fn condition(&mut self, spend: &mut SpendConditions, c: &Condition) {
        match c {
            Condition::AssertMyCoinId(_) => {
                spend.flags &= !ELIGIBLE_FOR_FF;
            }
            Condition::AssertMyParentId(_) => {
                // the singleton_top_layer_v1_1.clsp will only emit two
                // conditions, ASSERT_MY_AMOUNT and ASSERT_MY_PARENT_ID (in that
                // order). So we expect this conditon as the second in the list.
                // Any other conditions of this kind have to have been produced
                // by the inner puzzle, which we don't have control over. So in
                // that case this spend is not eligible for fast-forward.
                if self.condition_counter != 1 {
                    spend.flags &= !ELIGIBLE_FOR_FF;
                }
            }
            Condition::AggSigMe(_, _)
            | Condition::AggSigParent(_, _)
            | Condition::AggSigParentAmount(_, _)
            | Condition::AggSigParentPuzzle(_, _) => {
                spend.flags &= !ELIGIBLE_FOR_DEDUP;
                spend.flags &= !ELIGIBLE_FOR_FF;
            }
            Condition::AggSigPuzzle(_, _)
            | Condition::AggSigAmount(_, _)
            | Condition::AggSigPuzzleAmount(_, _)
            | Condition::AggSigUnsafe(_, _) => {
                spend.flags &= !ELIGIBLE_FOR_DEDUP;
            }
            Condition::SendMessage(src_mode, _dst, _msg) => {
                if (src_mode & super::messages::PARENT) != 0 {
                    spend.flags &= !ELIGIBLE_FOR_FF;
                }
                // de-duplicating a coin spend that's sending a message may
                // leave a receiver without a message, which is a failure
                spend.flags &= !ELIGIBLE_FOR_DEDUP;
            }
            Condition::ReceiveMessage(_src, dst_mode, _msg) => {
                if (dst_mode & super::messages::PARENT) != 0 {
                    spend.flags &= !ELIGIBLE_FOR_FF;
                }
                // de-duplicating a coin spend that's receiving a message may
                // leave a sent-message un-received, which is a failure
                spend.flags &= !ELIGIBLE_FOR_DEDUP;
            }
            _ => {}
        }
        self.condition_counter += 1;
    }

    fn post_spend(&mut self, a: &Allocator, spend: &mut SpendConditions) {
        // if this still looks like it might be a singleton, check the output coins
        // to look for something that looks like a singleton output, with the same
        // puzzle hash as our input coin
        if (spend.flags & ELIGIBLE_FOR_FF) != 0
            && !spend.create_coin.iter().any(|c| {
                (c.amount & 1) == 1
                    && a.atom(spend.puzzle_hash).as_ref() == c.puzzle_hash.as_slice()
            })
        {
            spend.flags &= !ELIGIBLE_FOR_FF;
        }
    }
}

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

#[derive(Debug)]
pub enum Condition {
    // pubkey (48 bytes) and message (<= 1024 bytes)
    AggSigUnsafe(NodePtr, NodePtr),
    AggSigMe(NodePtr, NodePtr),
    AggSigParent(NodePtr, NodePtr),
    AggSigPuzzle(NodePtr, NodePtr),
    AggSigAmount(NodePtr, NodePtr),
    AggSigPuzzleAmount(NodePtr, NodePtr),
    AggSigParentAmount(NodePtr, NodePtr),
    AggSigParentPuzzle(NodePtr, NodePtr),
    // puzzle hash (32 bytes), amount-node, amount integer, hint is an optional
    // hash (32 bytes), may be left as nil
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
    AssertEphemeral,

    // The softfork condition is one that we don't understand, it just applies
    // the specified cost
    Softfork(Cost),

    // source, destination, message
    SendMessage(u8, SpendId, NodePtr),
    ReceiveMessage(SpendId, u8, NodePtr),

    // this means the condition is unconditionally true and can be skipped
    Skip,
    SkipRelativeCondition,
}

fn check_agg_sig_unsafe_message(
    a: &Allocator,
    msg: NodePtr,
    constants: &ConsensusConstants,
) -> Result<(), ValidationErr> {
    if a.atom_len(msg) < 32 {
        return Ok(());
    }
    let buf = a.atom(msg);
    for additional_data in &[
        constants.agg_sig_me_additional_data.as_ref(),
        constants.agg_sig_parent_additional_data.as_ref(),
        constants.agg_sig_puzzle_additional_data.as_ref(),
        constants.agg_sig_amount_additional_data.as_ref(),
        constants.agg_sig_puzzle_amount_additional_data.as_ref(),
        constants.agg_sig_parent_amount_additional_data.as_ref(),
        constants.agg_sig_parent_puzzle_additional_data.as_ref(),
    ] {
        if buf.as_ref().ends_with(additional_data) {
            return Err(ValidationErr(msg, ErrorCode::InvalidMessage));
        }
    }
    Ok(())
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
        AGG_SIG_UNSAFE
        | AGG_SIG_ME
        | AGG_SIG_PUZZLE
        | AGG_SIG_PUZZLE_AMOUNT
        | AGG_SIG_PARENT
        | AGG_SIG_AMOUNT
        | AGG_SIG_PARENT_PUZZLE
        | AGG_SIG_PARENT_AMOUNT => {
            let pubkey = sanitize_hash(a, first(a, c)?, 48, ErrorCode::InvalidPublicKey)?;
            c = rest(a, c)?;
            let message = sanitize_announce_msg(a, first(a, c)?, ErrorCode::InvalidMessage)?;
            // AGG_SIG_* take two parameters

            if (flags & STRICT_ARGS_COUNT) != 0 {
                check_nil(a, rest(a, c)?)?;
            }
            match op {
                AGG_SIG_UNSAFE => Ok(Condition::AggSigUnsafe(pubkey, message)),
                AGG_SIG_ME => Ok(Condition::AggSigMe(pubkey, message)),
                AGG_SIG_PARENT => Ok(Condition::AggSigParent(pubkey, message)),
                AGG_SIG_PUZZLE => Ok(Condition::AggSigPuzzle(pubkey, message)),
                AGG_SIG_AMOUNT => Ok(Condition::AggSigAmount(pubkey, message)),
                AGG_SIG_PUZZLE_AMOUNT => Ok(Condition::AggSigPuzzleAmount(pubkey, message)),
                AGG_SIG_PARENT_AMOUNT => Ok(Condition::AggSigParentAmount(pubkey, message)),
                AGG_SIG_PARENT_PUZZLE => Ok(Condition::AggSigParentPuzzle(pubkey, message)),
                _ => {
                    panic!("unexpected");
                }
            }
        }
        CREATE_COIN => {
            let puzzle_hash = sanitize_hash(a, first(a, c)?, 32, ErrorCode::InvalidPuzzleHash)?;
            c = rest(a, c)?;
            let node = first(a, c)?;
            let amount = match sanitize_uint(a, node, 8, ErrorCode::InvalidCoinAmount)? {
                SanitizedUint::PositiveOverflow => {
                    return Err(ValidationErr(node, ErrorCode::CoinAmountExceedsMaximum));
                }
                SanitizedUint::NegativeOverflow => {
                    return Err(ValidationErr(node, ErrorCode::CoinAmountNegative));
                }
                SanitizedUint::Ok(amount) => amount,
            };
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
                    if let SExp::Atom = a.sexp(param) {
                        if a.atom_len(param) <= 32 {
                            return Ok(Condition::CreateCoin(puzzle_hash, amount, param));
                        }
                    }
                }
            } else if (flags & STRICT_ARGS_COUNT) != 0 {
                check_nil(a, c)?;
            }
            Ok(Condition::CreateCoin(puzzle_hash, amount, a.nil()))
        }
        SOFTFORK => {
            if (flags & NO_UNKNOWN_CONDS) != 0 {
                // We don't know of any new softforked-in conditions, so they
                // are all unknown
                Err(ValidationErr(c, ErrorCode::InvalidConditionOpcode))
            } else {
                match sanitize_uint(a, first(a, c)?, 4, ErrorCode::InvalidSoftforkCost)? {
                    // the first argument represents the cost of the condition.
                    // We scale it by 10000 to make the argument be a bit smaller
                    SanitizedUint::Ok(cost) => Ok(Condition::Softfork(cost * 10000)),
                    _ => Err(ValidationErr(c, ErrorCode::InvalidSoftforkCost)),
                }
            }
        }
        256..=65535 => {
            // All of these conditions are unknown
            // but they have costs
            if (flags & NO_UNKNOWN_CONDS) != 0 {
                Err(ValidationErr(c, ErrorCode::InvalidConditionOpcode))
            } else {
                Ok(Condition::Softfork(compute_unknown_condition_cost(op)))
            }
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
            let id = sanitize_hash(a, first(a, c)?, 32, ErrorCode::AssertMyPuzzleHashFailed)?;
            Ok(Condition::AssertMyPuzzlehash(id))
        }
        ASSERT_MY_AMOUNT => {
            maybe_check_args_terminator(a, c, flags)?;
            let amount = parse_amount(a, first(a, c)?, ErrorCode::AssertMyAmountFailed)?;
            Ok(Condition::AssertMyAmount(amount))
        }
        ASSERT_MY_BIRTH_SECONDS => {
            maybe_check_args_terminator(a, c, flags)?;
            let node = first(a, c)?;
            let code = ErrorCode::AssertMyBirthSecondsFailed;
            match sanitize_uint(a, node, 8, code)? {
                SanitizedUint::PositiveOverflow | SanitizedUint::NegativeOverflow => {
                    Err(ValidationErr(node, code))
                }
                SanitizedUint::Ok(r) => Ok(Condition::AssertMyBirthSeconds(r)),
            }
        }
        ASSERT_MY_BIRTH_HEIGHT => {
            maybe_check_args_terminator(a, c, flags)?;
            let node = first(a, c)?;
            let code = ErrorCode::AssertMyBirthHeightFailed;
            match sanitize_uint(a, node, 4, code)? {
                SanitizedUint::PositiveOverflow | SanitizedUint::NegativeOverflow => {
                    Err(ValidationErr(node, code))
                }
                SanitizedUint::Ok(r) => Ok(Condition::AssertMyBirthHeight(r as u32)),
            }
        }
        ASSERT_EPHEMERAL => {
            // this condition does not take any parameters
            if (flags & STRICT_ARGS_COUNT) != 0 {
                check_nil(a, c)?;
            }
            Ok(Condition::AssertEphemeral)
        }
        ASSERT_SECONDS_RELATIVE => {
            maybe_check_args_terminator(a, c, flags)?;
            let node = first(a, c)?;
            let code = ErrorCode::AssertSecondsRelativeFailed;
            match sanitize_uint(a, node, 8, code)? {
                SanitizedUint::PositiveOverflow => Err(ValidationErr(node, code)),
                SanitizedUint::NegativeOverflow => Ok(Condition::SkipRelativeCondition),
                SanitizedUint::Ok(r) => Ok(Condition::AssertSecondsRelative(r)),
            }
        }
        ASSERT_SECONDS_ABSOLUTE => {
            maybe_check_args_terminator(a, c, flags)?;
            let node = first(a, c)?;
            let code = ErrorCode::AssertSecondsAbsoluteFailed;
            match sanitize_uint(a, node, 4, code)? {
                SanitizedUint::PositiveOverflow => Err(ValidationErr(node, code)),
                SanitizedUint::NegativeOverflow => Ok(Condition::Skip),
                SanitizedUint::Ok(r) => Ok(Condition::AssertSecondsAbsolute(r)),
            }
        }
        ASSERT_HEIGHT_RELATIVE => {
            maybe_check_args_terminator(a, c, flags)?;
            let node = first(a, c)?;
            let code = ErrorCode::AssertHeightRelativeFailed;
            match sanitize_uint(a, node, 4, code)? {
                SanitizedUint::PositiveOverflow => Err(ValidationErr(node, code)),
                SanitizedUint::NegativeOverflow => Ok(Condition::SkipRelativeCondition),
                SanitizedUint::Ok(r) => Ok(Condition::AssertHeightRelative(r as u32)),
            }
        }
        ASSERT_HEIGHT_ABSOLUTE => {
            maybe_check_args_terminator(a, c, flags)?;
            let node = first(a, c)?;
            let code = ErrorCode::AssertHeightAbsoluteFailed;
            match sanitize_uint(a, node, 4, code)? {
                SanitizedUint::PositiveOverflow => Err(ValidationErr(node, code)),
                SanitizedUint::NegativeOverflow => Ok(Condition::Skip),
                SanitizedUint::Ok(r) => Ok(Condition::AssertHeightAbsolute(r as u32)),
            }
        }
        ASSERT_BEFORE_SECONDS_RELATIVE => {
            maybe_check_args_terminator(a, c, flags)?;
            let node = first(a, c)?;
            let code = ErrorCode::AssertBeforeSecondsRelativeFailed;
            match sanitize_uint(a, node, 8, code)? {
                SanitizedUint::PositiveOverflow => Ok(Condition::SkipRelativeCondition),
                SanitizedUint::NegativeOverflow => Err(ValidationErr(node, code)),
                SanitizedUint::Ok(r) => Ok(Condition::AssertBeforeSecondsRelative(r)),
            }
        }
        ASSERT_BEFORE_SECONDS_ABSOLUTE => {
            maybe_check_args_terminator(a, c, flags)?;

            let node = first(a, c)?;
            let code = ErrorCode::AssertBeforeSecondsAbsoluteFailed;
            match sanitize_uint(a, node, 8, code)? {
                SanitizedUint::PositiveOverflow => Ok(Condition::Skip),
                SanitizedUint::NegativeOverflow => Err(ValidationErr(node, code)),
                SanitizedUint::Ok(r) => Ok(Condition::AssertBeforeSecondsAbsolute(r)),
            }
        }
        ASSERT_BEFORE_HEIGHT_RELATIVE => {
            maybe_check_args_terminator(a, c, flags)?;
            let node = first(a, c)?;
            let code = ErrorCode::AssertBeforeHeightRelativeFailed;
            match sanitize_uint(a, node, 4, code)? {
                SanitizedUint::PositiveOverflow => Ok(Condition::SkipRelativeCondition),
                SanitizedUint::NegativeOverflow => Err(ValidationErr(node, code)),
                SanitizedUint::Ok(r) => Ok(Condition::AssertBeforeHeightRelative(r as u32)),
            }
        }
        ASSERT_BEFORE_HEIGHT_ABSOLUTE => {
            maybe_check_args_terminator(a, c, flags)?;
            let node = first(a, c)?;
            let code = ErrorCode::AssertBeforeHeightAbsoluteFailed;
            match sanitize_uint(a, node, 4, code)? {
                SanitizedUint::PositiveOverflow => Ok(Condition::Skip),
                SanitizedUint::NegativeOverflow => Err(ValidationErr(node, code)),
                SanitizedUint::Ok(r) => Ok(Condition::AssertBeforeHeightAbsolute(r as u32)),
            }
        }
        SEND_MESSAGE => {
            let mode = sanitize_message_mode(a, first(a, c)?)?;
            c = rest(a, c)?;
            let message = sanitize_announce_msg(a, first(a, c)?, ErrorCode::InvalidMessage)?;
            c = rest(a, c)?;
            let dst = SpendId::parse(a, &mut c, (mode & 0b111) as u8)?;

            if (flags & STRICT_ARGS_COUNT) != 0 {
                check_nil(a, c)?;
            }

            Ok(Condition::SendMessage(
                ((mode >> 3) & 0b111) as u8,
                dst,
                message,
            ))
        }
        RECEIVE_MESSAGE => {
            let mode = sanitize_message_mode(a, first(a, c)?)?;
            c = rest(a, c)?;
            let message = sanitize_announce_msg(a, first(a, c)?, ErrorCode::InvalidMessage)?;
            c = rest(a, c)?;
            let src = SpendId::parse(a, &mut c, ((mode >> 3) & 0b111) as u8)?;

            if (flags & STRICT_ARGS_COUNT) != 0 {
                check_nil(a, c)?;
            }
            Ok(Condition::ReceiveMessage(
                src,
                (mode & 0b111) as u8,
                message,
            ))
        }
        REMARK => {
            // this condition is always true, we always ignore arguments
            Ok(Condition::Skip)
        }
        _ => Err(ValidationErr(c, ErrorCode::InvalidConditionOpcode)),
    }
}

#[derive(Debug, Clone)]
pub struct NewCoin {
    pub puzzle_hash: Bytes32,
    pub amount: u64,
    // the hint is optional. When not provided, this points to nil (NodePtr
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

// These are all the conditions related directly to a specific spend.
#[derive(Debug, Clone)]
pub struct SpendConditions {
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
    // all these integers are initialized to None, which also means "no
    // constraint". A 0 may impose a constraint (post 1.8.0 soft-fork).
    // for height_relative, 0 has always imposed a constraint (for ephemeral spends)
    // negative values are ignored and not passed up to the next layer.
    pub height_relative: Option<u32>,
    pub seconds_relative: Option<u64>,
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
    // Maybe this should be an array of vectors
    pub agg_sig_me: Vec<(PublicKey, NodePtr)>,
    pub agg_sig_parent: Vec<(PublicKey, NodePtr)>,
    pub agg_sig_puzzle: Vec<(PublicKey, NodePtr)>,
    pub agg_sig_amount: Vec<(PublicKey, NodePtr)>,
    pub agg_sig_puzzle_amount: Vec<(PublicKey, NodePtr)>,
    pub agg_sig_parent_amount: Vec<(PublicKey, NodePtr)>,
    pub agg_sig_parent_puzzle: Vec<(PublicKey, NodePtr)>,
    // Flags describing properties of this spend. See flags above
    pub flags: u32,
}

impl SpendConditions {
    pub fn new(
        parent_id: NodePtr,
        coin_amount: u64,
        puzzle_hash: NodePtr,
        coin_id: Arc<Bytes32>,
    ) -> SpendConditions {
        SpendConditions {
            parent_id,
            coin_amount,
            puzzle_hash,
            coin_id,
            height_relative: None,
            seconds_relative: None,
            before_height_relative: None,
            before_seconds_relative: None,
            birth_height: None,
            birth_seconds: None,
            create_coin: HashSet::new(),
            agg_sig_me: Vec::new(),
            agg_sig_parent: Vec::new(),
            agg_sig_puzzle: Vec::new(),
            agg_sig_amount: Vec::new(),
            agg_sig_puzzle_amount: Vec::new(),
            agg_sig_parent_amount: Vec::new(),
            agg_sig_parent_puzzle: Vec::new(),
            flags: 0,
        }
    }
}

// these are all the conditions and properties of a complete spend bundle.
// some conditions that are created by individual spends are aggregated at the
// spend bundle level, like reserve_fee and absolute time locks. Other
// conditions are per spend, like relative time-locks and create coins (because
// they have an implied parent coin ID).
#[derive(Debug, Default)]
pub struct SpendBundleConditions {
    pub spends: Vec<SpendConditions>,
    // conditions
    // all these integers are initialized to 0, which also means "no
    // constraint". i.e. a 0 in these conditions are inherently satisified and
    // ignored. 0 (or negative values) are not passed up to the next layer
    // The sum of all reserve fee conditions
    pub reserve_fee: u64,
    // the highest height/time conditions (i.e. most strict). 0 values are no-ops
    pub height_absolute: u32,
    pub seconds_absolute: u64,
    // Unsafe Agg Sig conditions (i.e. not tied to the spend generating it)
    pub agg_sig_unsafe: Vec<(PublicKey, NodePtr)>,
    // when set, this is the lowest (i.e. most restrictive) of all
    // ASSERT_BEFORE_HEIGHT_ABSOLUTE conditions
    pub before_height_absolute: Option<u32>,
    // ASSERT_BEFORE_SECONDS_ABSOLUTE conditions
    pub before_seconds_absolute: Option<u64>,

    // the cost of conditions (when returned by parse_spends())
    // run_block_generator() will include CLVM cost and byte cost (making this
    // the total cost)
    pub cost: u64,

    // the sum of all values of all spent coins
    pub removal_amount: u128,

    // the sum of all amounts of CREATE_COIN conditions
    pub addition_amount: u128,

    // true if the block/spend bundle aggregate signature was validated
    pub validated_signature: bool,
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

    // These are just list of all the messages being sent or received. There's
    // no deduplication. We defer resolving and checking the messages until
    // after we're done parsing all conditions for all spends
    messages: Vec<Message>,

    // the assert concurrent spend coin IDs are inserted into this set and
    // checked once everything has been parsed.
    assert_concurrent_spend: HashSet<NodePtr>,

    // the assert concurrent puzzle hashes are inserted into this set and
    // checked once everything has been parsed.
    assert_concurrent_puzzle: HashSet<NodePtr>,

    // all coin IDs that have been spent so far. When we parse a spend we also
    // compute the coin ID, and stick it in this map. It's reference counted
    // since it may also be referenced by announcements. The value mapped to is
    // the index of the spend in SpendBundleConditions::spends
    spent_coins: HashMap<Arc<Bytes32>, usize>,

    // for every coin spent, we also store all the puzzle hashes that were
    // spent. Note that these are just the node pointers into the allocator, so
    // there may still be duplicates here. We defer creating a hash set of the
    // actual hashes until the end, and only if there are any puzzle assertions
    spent_puzzles: HashSet<NodePtr>,

    // we record all coins that assert that they are ephemeral in here. Once
    // we've processed all spends, we ensure that all of these coins were
    // created in this same block
    // each item is the index into the SpendBundleConditions::spends vector
    assert_ephemeral: HashSet<usize>,

    // spends that use relative height- or time conditions are disallowed on
    // ephemeral coins. They are recorded in this set to be be checked once all
    // spends have been parsed. These conditions are:
    // ASSERT_HEIGHT_RELATIVE
    // ASSERT_SECONDS_RELATIVE
    // ASSERT_BEFORE_HEIGHT_RELATIVE
    // ASSERT_BEFORE_SECONDS_RELATIVE
    // ASSERT_MY_BIRTH_SECONDS
    // ASSERT_MY_BIRTH_HEIGHT
    // each item is the index into the SpendBundleConditions::spends vector
    assert_not_ephemeral: HashSet<usize>,

    // All public keys and messages emitted by the generator. We'll validate
    // these against the aggregate signature at the end, unless the
    // DONT_VALIDATE_SIGNATURE flag is set
    // TODO: We would probably save heap allocations by turning this into a
    // blst_pairing object.
    pub pkm_pairs: Vec<(PublicKey, Bytes)>,
}

// returns (parent-id, puzzle-hash, amount, condition-list)
pub(crate) fn parse_single_spend(
    a: &Allocator,
    mut spend: NodePtr,
) -> Result<(NodePtr, NodePtr, NodePtr, NodePtr), ValidationErr> {
    let parent_id = first(a, spend)?;
    spend = rest(a, spend)?;
    let puzzle_hash = first(a, spend)?;
    spend = rest(a, spend)?;
    let amount = first(a, spend)?;
    spend = rest(a, spend)?;
    let cond = first(a, spend)?;
    // the rest() here is spend_level_extr. Typically nil
    Ok((parent_id, puzzle_hash, amount, cond))
}

#[allow(clippy::too_many_arguments)]
pub fn process_single_spend<V: SpendVisitor>(
    a: &Allocator,
    ret: &mut SpendBundleConditions,
    state: &mut ParseState,
    parent_id: NodePtr,
    puzzle_hash: NodePtr,
    amount: NodePtr,
    conditions: NodePtr,
    flags: u32,
    max_cost: &mut Cost,
    constants: &ConsensusConstants,
) -> Result<(), ValidationErr> {
    let parent_id = sanitize_hash(a, parent_id, 32, ErrorCode::InvalidParentId)?;
    let puzzle_hash = sanitize_hash(a, puzzle_hash, 32, ErrorCode::InvalidPuzzleHash)?;
    let my_amount = parse_amount(a, amount, ErrorCode::InvalidCoinAmount)?;
    let amount_buf = a.atom(amount);

    let coin_id = Arc::new(compute_coin_id(
        a,
        parent_id,
        puzzle_hash,
        amount_buf.as_ref(),
    ));

    if state
        .spent_coins
        .insert(coin_id.clone(), ret.spends.len())
        .is_some()
    {
        // if this coin ID has already been added to this set, it's a double
        // spend
        return Err(ValidationErr(parent_id, ErrorCode::DoubleSpend));
    }

    state.spent_puzzles.insert(puzzle_hash);

    ret.removal_amount += my_amount as u128;

    let mut spend = SpendConditions::new(parent_id, my_amount, puzzle_hash, coin_id);

    let mut visitor = V::new_spend(&mut spend);

    parse_conditions(
        a,
        ret,
        state,
        spend,
        conditions,
        flags,
        max_cost,
        constants,
        &mut visitor,
    )
}

fn assert_not_ephemeral(spend_flags: &mut u32, state: &mut ParseState, idx: usize) {
    if (*spend_flags & HAS_RELATIVE_CONDITION) != 0 {
        return;
    }

    state.assert_not_ephemeral.insert(idx);
    *spend_flags |= HAS_RELATIVE_CONDITION;
}

fn decrement(cnt: &mut u32, n: NodePtr) -> Result<(), ValidationErr> {
    if *cnt == 0 {
        Err(ValidationErr(n, ErrorCode::TooManyAnnouncements))
    } else {
        *cnt -= 1;
        Ok(())
    }
}

fn to_key(a: &Allocator, pk: NodePtr) -> Result<PublicKey, ValidationErr> {
    let key = PublicKey::from_bytes(a.atom(pk).as_ref().try_into().expect("internal error"))
        .map_err(|_| ValidationErr(pk, ErrorCode::InvalidPublicKey))?;
    if key.is_inf() {
        Err(ValidationErr(pk, ErrorCode::InvalidPublicKey))
    } else {
        Ok(key)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn parse_conditions<V: SpendVisitor>(
    a: &Allocator,
    ret: &mut SpendBundleConditions,
    state: &mut ParseState,
    mut spend: SpendConditions,
    mut iter: NodePtr,
    flags: u32,
    max_cost: &mut Cost,
    constants: &ConsensusConstants,
    visitor: &mut V,
) -> Result<(), ValidationErr> {
    let mut announce_countdown: u32 = 1024;

    while let Some((mut c, next)) = next(a, iter)? {
        iter = next;
        let Some(op) = parse_opcode(a, first(a, c)?, flags) else {
            // in strict mode we don't allow unknown conditions
            if (flags & NO_UNKNOWN_CONDS) != 0 {
                return Err(ValidationErr(c, ErrorCode::InvalidConditionOpcode));
            }
            // in non-strict mode, we just ignore unknown conditions
            continue;
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
            AGG_SIG_UNSAFE
            | AGG_SIG_ME
            | AGG_SIG_PUZZLE
            | AGG_SIG_PUZZLE_AMOUNT
            | AGG_SIG_PARENT
            | AGG_SIG_AMOUNT
            | AGG_SIG_PARENT_PUZZLE
            | AGG_SIG_PARENT_AMOUNT => {
                if *max_cost < AGG_SIG_COST {
                    return Err(ValidationErr(c, ErrorCode::CostExceeded));
                }
                *max_cost -= AGG_SIG_COST;
            }
            _ => (),
        }
        c = rest(a, c)?;
        let cva = parse_args(a, c, op, flags)?;
        visitor.condition(&mut spend, &cva);
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
                    puzzle_hash: a.atom(ph).as_ref().try_into().unwrap(),
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
                if let Some(existing) = spend.seconds_relative {
                    spend.seconds_relative = Some(max(existing, s));
                } else {
                    spend.seconds_relative = Some(s);
                }
                if let Some(bs) = spend.before_seconds_relative {
                    if bs <= s {
                        // this spend bundle requres to be spent *before* a
                        // timestamp and also *after* a timestamp that's the
                        // same or later. that's impossible.
                        return Err(ValidationErr(
                            c,
                            ErrorCode::ImpossibleSecondsRelativeConstraints,
                        ));
                    }
                }
                assert_not_ephemeral(&mut spend.flags, state, ret.spends.len());
            }
            Condition::AssertSecondsAbsolute(s) => {
                // keep the most strict condition. i.e. the highest limit
                ret.seconds_absolute = max(ret.seconds_absolute, s);
            }
            Condition::AssertHeightRelative(h) => {
                // keep the most strict condition. i.e. the highest limit
                if let Some(existing) = spend.height_relative {
                    spend.height_relative = Some(max(existing, h));
                } else {
                    spend.height_relative = Some(h);
                }
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
                assert_not_ephemeral(&mut spend.flags, state, ret.spends.len());
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
                if let Some(sr) = spend.seconds_relative {
                    if s <= sr {
                        // this spend bundle requres to be spent *before* a
                        // timestamp and also *after* a timestamp that's the
                        // same or later. that's impossible.
                        return Err(ValidationErr(
                            c,
                            ErrorCode::ImpossibleSecondsRelativeConstraints,
                        ));
                    }
                }
                assert_not_ephemeral(&mut spend.flags, state, ret.spends.len());
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
                assert_not_ephemeral(&mut spend.flags, state, ret.spends.len());
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
                if a.atom(id).as_ref() != (*spend.coin_id).as_ref() {
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
                assert_not_ephemeral(&mut spend.flags, state, ret.spends.len());
            }
            Condition::AssertMyBirthHeight(h) => {
                // if this spend already has a birth_height assertion, it's an
                // error if it's different from the new birth assertion. One of
                // them must be false
                if spend.birth_height.map(|v| v == h) == Some(false) {
                    return Err(ValidationErr(c, ErrorCode::AssertMyBirthHeightFailed));
                }
                spend.birth_height = Some(h);
                assert_not_ephemeral(&mut spend.flags, state, ret.spends.len());
            }
            Condition::AssertEphemeral => {
                state.assert_ephemeral.insert(ret.spends.len());
            }
            Condition::AssertMyParentId(id) => {
                if a.atom(id).as_ref() != a.atom(spend.parent_id).as_ref() {
                    return Err(ValidationErr(c, ErrorCode::AssertMyParentIdFailed));
                }
            }
            Condition::AssertMyPuzzlehash(hash) => {
                if a.atom(hash).as_ref() != a.atom(spend.puzzle_hash).as_ref() {
                    return Err(ValidationErr(c, ErrorCode::AssertMyPuzzleHashFailed));
                }
            }
            Condition::CreateCoinAnnouncement(msg) => {
                decrement(&mut announce_countdown, msg)?;
                state.announce_coin.insert((spend.coin_id.clone(), msg));
            }
            Condition::CreatePuzzleAnnouncement(msg) => {
                decrement(&mut announce_countdown, msg)?;
                state.announce_puzzle.insert((spend.puzzle_hash, msg));
            }
            Condition::AssertCoinAnnouncement(msg) => {
                decrement(&mut announce_countdown, msg)?;
                state.assert_coin.insert(msg);
            }
            Condition::AssertPuzzleAnnouncement(msg) => {
                decrement(&mut announce_countdown, msg)?;
                state.assert_puzzle.insert(msg);
            }
            Condition::AssertConcurrentSpend(id) => {
                decrement(&mut announce_countdown, id)?;
                state.assert_concurrent_spend.insert(id);
            }
            Condition::AssertConcurrentPuzzle(id) => {
                decrement(&mut announce_countdown, id)?;
                state.assert_concurrent_puzzle.insert(id);
            }
            Condition::AggSigMe(pk, msg) => {
                spend.agg_sig_me.push((to_key(a, pk)?, msg));
                if (flags & DONT_VALIDATE_SIGNATURE) == 0 {
                    let mut msg = a.atom(msg).as_ref().to_vec();
                    msg.extend((*spend.coin_id).as_slice());
                    msg.extend(constants.agg_sig_me_additional_data.as_slice());
                    state.pkm_pairs.push((to_key(a, pk)?, msg.into()));
                }
            }
            Condition::AggSigParent(pk, msg) => {
                spend.agg_sig_parent.push((to_key(a, pk)?, msg));
                if (flags & DONT_VALIDATE_SIGNATURE) == 0 {
                    let mut msg = a.atom(msg).as_ref().to_vec();
                    msg.extend(a.atom(spend.parent_id).as_ref());
                    msg.extend(constants.agg_sig_parent_additional_data.as_slice());
                    state.pkm_pairs.push((to_key(a, pk)?, msg.into()));
                }
            }
            Condition::AggSigPuzzle(pk, msg) => {
                spend.agg_sig_puzzle.push((to_key(a, pk)?, msg));
                if (flags & DONT_VALIDATE_SIGNATURE) == 0 {
                    let mut msg = a.atom(msg).as_ref().to_vec();
                    msg.extend(a.atom(spend.puzzle_hash).as_ref());
                    msg.extend(constants.agg_sig_puzzle_additional_data.as_slice());
                    state.pkm_pairs.push((to_key(a, pk)?, msg.into()));
                }
            }
            Condition::AggSigAmount(pk, msg) => {
                spend.agg_sig_amount.push((to_key(a, pk)?, msg));
                if (flags & DONT_VALIDATE_SIGNATURE) == 0 {
                    let mut msg = a.atom(msg).as_ref().to_vec();
                    msg.extend(u64_to_bytes(spend.coin_amount).as_slice());
                    msg.extend(constants.agg_sig_amount_additional_data.as_slice());
                    state.pkm_pairs.push((to_key(a, pk)?, msg.into()));
                }
            }
            Condition::AggSigPuzzleAmount(pk, msg) => {
                spend.agg_sig_puzzle_amount.push((to_key(a, pk)?, msg));
                if (flags & DONT_VALIDATE_SIGNATURE) == 0 {
                    let mut msg = a.atom(msg).as_ref().to_vec();
                    msg.extend(a.atom(spend.puzzle_hash).as_ref());
                    msg.extend(u64_to_bytes(spend.coin_amount).as_slice());
                    msg.extend(constants.agg_sig_puzzle_amount_additional_data.as_slice());
                    state.pkm_pairs.push((to_key(a, pk)?, msg.into()));
                }
            }
            Condition::AggSigParentAmount(pk, msg) => {
                spend.agg_sig_parent_amount.push((to_key(a, pk)?, msg));
                if (flags & DONT_VALIDATE_SIGNATURE) == 0 {
                    let mut msg = a.atom(msg).as_ref().to_vec();
                    msg.extend(a.atom(spend.parent_id).as_ref());
                    msg.extend(u64_to_bytes(spend.coin_amount).as_slice());
                    msg.extend(constants.agg_sig_parent_amount_additional_data.as_slice());
                    state.pkm_pairs.push((to_key(a, pk)?, msg.into()));
                }
            }
            Condition::AggSigParentPuzzle(pk, msg) => {
                spend.agg_sig_parent_puzzle.push((to_key(a, pk)?, msg));
                if (flags & DONT_VALIDATE_SIGNATURE) == 0 {
                    let mut msg = a.atom(msg).as_ref().to_vec();
                    msg.extend(a.atom(spend.parent_id).as_ref());
                    msg.extend(a.atom(spend.puzzle_hash).as_ref());
                    msg.extend(constants.agg_sig_parent_puzzle_additional_data.as_slice());
                    state.pkm_pairs.push((to_key(a, pk)?, msg.into()));
                }
            }
            Condition::AggSigUnsafe(pk, msg) => {
                // AGG_SIG_UNSAFE messages are not allowed to end with the
                // suffix added to other AGG_SIG_* conditions
                check_agg_sig_unsafe_message(a, msg, constants)?;
                ret.agg_sig_unsafe.push((to_key(a, pk)?, msg));
                if (flags & DONT_VALIDATE_SIGNATURE) == 0 {
                    state
                        .pkm_pairs
                        .push((to_key(a, pk)?, a.atom(msg).as_ref().to_vec().into()));
                }
            }
            Condition::Softfork(cost) => {
                if *max_cost < cost {
                    return Err(ValidationErr(c, ErrorCode::CostExceeded));
                }
                *max_cost -= cost;
            }
            Condition::SendMessage(src_mode, dst, msg) => {
                decrement(&mut announce_countdown, msg)?;
                let src = SpendId::from_self(
                    src_mode,
                    spend.parent_id,
                    spend.puzzle_hash,
                    spend.coin_amount,
                    &spend.coin_id,
                )?;
                state.messages.push(Message {
                    src,
                    dst,
                    msg,
                    counter: 1,
                });
            }
            Condition::ReceiveMessage(src, dst_mode, msg) => {
                decrement(&mut announce_countdown, msg)?;
                let dst = SpendId::from_self(
                    dst_mode,
                    spend.parent_id,
                    spend.puzzle_hash,
                    spend.coin_amount,
                    &spend.coin_id,
                )?;
                state.messages.push(Message {
                    src,
                    dst,
                    msg,
                    counter: -1,
                });
            }
            Condition::SkipRelativeCondition => {
                assert_not_ephemeral(&mut spend.flags, state, ret.spends.len());
            }
            Condition::Skip => {}
        }
    }

    visitor.post_spend(a, &mut spend);

    ret.spends.push(spend);
    Ok(())
}

fn is_ephemeral(
    a: &Allocator,
    spend_idx: usize,
    spent_ids: &HashMap<Arc<Bytes32>, usize>,
    spends: &[SpendConditions],
) -> bool {
    let spend = &spends[spend_idx];
    let idx = match spent_ids.get(&Bytes32::try_from(a.atom(spend.parent_id).as_ref()).unwrap()) {
        None => {
            return false;
        }
        Some(idx) => *idx,
    };

    // then lookup the coin (puzzle hash, amount) in its set of created
    // coins. Note that hint is not relevant for this lookup
    let parent_spend = &spends[idx];
    parent_spend.create_coin.contains(&NewCoin {
        puzzle_hash: Bytes32::try_from(a.atom(spend.puzzle_hash).as_ref()).unwrap(),
        amount: spend.coin_amount,
        hint: a.nil(),
    })
}

// This function parses, and validates aspects of, the above structure and
// returns a list of all spends, along with all conditions, organized by
// condition op-code
pub fn parse_spends<V: SpendVisitor>(
    a: &Allocator,
    spends: NodePtr,
    max_cost: Cost,
    flags: u32,
    aggregate_signature: &Signature,
    bls_cache: Option<&mut BlsCache>,
    constants: &ConsensusConstants,
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
        let (parent_id, puzzle_hash, amount, conds) = parse_single_spend(a, spend)?;

        process_single_spend::<V>(
            a,
            &mut ret,
            &mut state,
            parent_id,
            puzzle_hash,
            amount,
            conds,
            flags,
            &mut cost_left,
            constants,
        )?;
    }

    validate_conditions(a, &ret, &state, spends, flags)?;
    validate_signature(&state, aggregate_signature, flags, bls_cache)?;
    ret.validated_signature = (flags & DONT_VALIDATE_SIGNATURE) == 0;

    ret.cost = max_cost - cost_left;
    Ok(ret)
}

pub fn validate_conditions(
    a: &Allocator,
    ret: &SpendBundleConditions,
    state: &ParseState,
    spends: NodePtr,
    _flags: u32,
) -> Result<(), ValidationErr> {
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
    for coin_id in &state.assert_concurrent_spend {
        if !state
            .spent_coins
            .contains_key(&Bytes32::try_from(a.atom(*coin_id).as_ref()).unwrap())
        {
            return Err(ValidationErr(
                *coin_id,
                ErrorCode::AssertConcurrentSpendFailed,
            ));
        }
    }

    if !state.assert_concurrent_puzzle.is_empty() {
        let mut spent_phs = HashSet::<Bytes32>::new();

        // expand all the spent puzzle hashes into a set, to allow
        // fast lookups of all assertions
        for ph in &state.spent_puzzles {
            spent_phs.insert(a.atom(*ph).as_ref().try_into().unwrap());
        }

        for puzzle_assert in &state.assert_concurrent_puzzle {
            if !spent_phs.contains(&a.atom(*puzzle_assert).as_ref().try_into().unwrap()) {
                return Err(ValidationErr(
                    *puzzle_assert,
                    ErrorCode::AssertConcurrentPuzzleFailed,
                ));
            }
        }
    }

    // check all the assert announcements
    // if there are no asserts, there is no need to hash all the announcements
    if !state.assert_coin.is_empty() {
        let mut announcements = HashSet::<Bytes32>::new();

        for (coin_id, announce) in &state.announce_coin {
            let mut hasher = Sha256::new();
            hasher.update(**coin_id);
            hasher.update(a.atom(*announce));
            let announcement_id: [u8; 32] = hasher.finalize();
            announcements.insert(announcement_id.into());
        }

        for coin_assert in &state.assert_coin {
            if !announcements.contains(&a.atom(*coin_assert).as_ref().try_into().unwrap()) {
                return Err(ValidationErr(
                    *coin_assert,
                    ErrorCode::AssertCoinAnnouncementFailed,
                ));
            }
        }
    }

    for spend_idx in &state.assert_ephemeral {
        // make sure this coin was created in this block
        if !is_ephemeral(a, *spend_idx, &state.spent_coins, &ret.spends) {
            return Err(ValidationErr(
                ret.spends[*spend_idx].parent_id,
                ErrorCode::AssertEphemeralFailed,
            ));
        }
    }

    for spend_idx in &state.assert_not_ephemeral {
        // make sure this coin was NOT created in this block
        // because consensus rules do not allow relative conditions on
        // ephemeral spends
        if is_ephemeral(a, *spend_idx, &state.spent_coins, &ret.spends) {
            return Err(ValidationErr(
                ret.spends[*spend_idx].parent_id,
                ErrorCode::EphemeralRelativeCondition,
            ));
        }
    }

    if !state.assert_puzzle.is_empty() {
        let mut announcements = HashSet::<Bytes32>::new();

        for (puzzle_hash, announce) in &state.announce_puzzle {
            let mut hasher = Sha256::new();
            hasher.update(a.atom(*puzzle_hash));
            hasher.update(a.atom(*announce));
            let announcement_id: [u8; 32] = hasher.finalize();
            announcements.insert(announcement_id.into());
        }

        for puzzle_assert in &state.assert_puzzle {
            if !announcements.contains(&a.atom(*puzzle_assert).as_ref().try_into().unwrap()) {
                return Err(ValidationErr(
                    *puzzle_assert,
                    ErrorCode::AssertPuzzleAnnouncementFailed,
                ));
            }
        }
    }

    if !state.messages.is_empty() {
        // the integers count the number of times the message has been sent
        // minus the number of times it's been received. At the end we ensure
        // all counters are 0, otherwise some message wasn't received or sent
        // the right number of times.
        let mut messages = HashMap::<Vec<u8>, i32>::new();

        for msg in &state.messages {
            *messages.entry(msg.make_key(a)).or_insert(0) += i32::from(msg.counter);
        }

        for count in messages.values() {
            if *count != 0 {
                return Err(ValidationErr(
                    NodePtr::NIL,
                    ErrorCode::MessageNotSentOrReceived,
                ));
            }
        }
    }

    // TODO: there may be more failures that can be detected early here, for
    // example an assert-my-birth-height that's incompatible assert-height or
    // assert-before-height. Same thing for the seconds counterpart

    Ok(())
}

pub fn validate_signature(
    state: &ParseState,
    signature: &Signature,
    flags: u32,
    bls_cache: Option<&mut BlsCache>,
) -> Result<(), ValidationErr> {
    if (flags & DONT_VALIDATE_SIGNATURE) != 0 {
        return Ok(());
    }

    if let Some(bls_cache) = bls_cache {
        if !bls_cache.aggregate_verify(
            state.pkm_pairs.iter().map(|(pk, msg)| (pk, msg.as_slice())),
            signature,
        ) {
            return Err(ValidationErr(
                NodePtr::NIL,
                ErrorCode::BadAggregateSignature,
            ));
        }
    } else if !aggregate_verify(
        signature,
        state.pkm_pairs.iter().map(|(pk, msg)| (pk, msg.as_slice())),
    ) {
        return Err(ValidationErr(
            NodePtr::NIL,
            ErrorCode::BadAggregateSignature,
        ));
    }
    Ok(())
}

#[cfg(test)]
use crate::consensus_constants::TEST_CONSTANTS;
#[cfg(test)]
use clvmr::number::Number;
#[cfg(test)]
use clvmr::serde::node_to_bytes;
#[cfg(test)]
use hex::FromHex;
#[cfg(test)]
use hex_literal::hex;
#[cfg(test)]
use num_traits::Num;
#[cfg(test)]
use rstest::rstest;

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
const PUBKEY: &[u8; 48] = &hex!("aefe1789d6476f60439e1168f588ea16652dc321279f05a805fbc63933e88ae9c175d6c6ab182e54af562e1a0dce41bb");
#[cfg(test)]
const SECRET_KEY: &[u8; 32] =
    &hex!("6fc9d9a2b05fd1f0e51bc91041a03be8657081f272ec281aff731624f0d1c220");
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
    let coin_id: [u8; 32] = hasher.finalize();
    coin_id.into()
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
type Callback = Option<Box<dyn Fn(&mut Allocator) -> NodePtr>>;

#[cfg(test)]
fn parse_list_impl(
    a: &mut Allocator,
    input: &str,
    callback: &Callback,
    subs: &HashMap<&'static str, NodePtr>,
) -> (NodePtr, usize) {
    // skip whitespace
    if let Some(rest) = input.strip_prefix(' ') {
        let (n, skip) = parse_list_impl(a, rest, callback, subs);
        return (n, skip + 1);
    }

    if input.starts_with(')') {
        (a.nil(), 1)
    } else if let Some(rest) = input.strip_prefix('(') {
        let (first, step1) = parse_list_impl(a, rest, callback, subs);
        let (rest, step2) = parse_list_impl(a, &input[(1 + step1)..], callback, subs);
        (a.new_pair(first, rest).unwrap(), 1 + step1 + step2)
    } else if let Some(rest) = input.strip_prefix('{') {
        // substitute '{X}' tokens with our test hashes and messages
        // this keeps the test cases a lot simpler
        let var = rest.split_once('}').unwrap().0;

        let ret = match var {
            "" => callback.as_ref().unwrap()(a),
            _ => *subs.get(var).unwrap(),
        };
        (ret, var.len() + 2)
    } else if input.starts_with("0x") {
        let v = input.split_once(' ').unwrap().0;

        let buf = Vec::from_hex(v.strip_prefix("0x").unwrap()).unwrap();
        (a.new_atom(&buf).unwrap(), v.len() + 1)
    } else if input.starts_with('-') || "0123456789".contains(input.get(0..1).unwrap()) {
        let v = input.split_once(' ').unwrap().0;
        let num = Number::from_str_radix(v, 10).unwrap();
        (a.new_number(num).unwrap(), v.len() + 1)
    } else {
        panic!("atom not supported \"{input}\"");
    }
}

#[cfg(test)]
fn parse_list(a: &mut Allocator, input: &str, callback: &Callback) -> NodePtr {
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
        "coin21_456",
        a.new_atom(&test_coin_id(H2, H1, 456)).unwrap(),
    );
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
#[allow(clippy::needless_pass_by_value)]
fn cond_test_cb(
    input: &str,
    flags: u32,
    callback: Callback,
    signature: &Signature,
    bls_cache: Option<&mut BlsCache>,
) -> Result<(Allocator, SpendBundleConditions), ValidationErr> {
    let mut a = Allocator::new();

    println!("input: {input}");

    let n = parse_list(&mut a, input, &callback);
    for c in node_to_bytes(&a, n).unwrap() {
        print!("{c:02x}");
    }
    println!();
    match parse_spends::<MempoolVisitor>(
        &a,
        n,
        11_000_000_000,
        flags,
        signature,
        bls_cache,
        &TEST_CONSTANTS,
    ) {
        Ok(list) => {
            for n in &list.spends {
                println!("{n:?}");
            }
            Ok((a, list))
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
use crate::gen::flags::MEMPOOL_MODE;

#[cfg(test)]
fn cond_test(input: &str) -> Result<(Allocator, SpendBundleConditions), ValidationErr> {
    // by default, run all tests in strict mempool mode
    cond_test_cb(input, MEMPOOL_MODE, None, &Signature::default(), None)
}

#[cfg(test)]
fn cond_test_flag(
    input: &str,
    flags: u32,
) -> Result<(Allocator, SpendBundleConditions), ValidationErr> {
    cond_test_cb(input, flags, None, &Signature::default(), None)
}

#[cfg(test)]
fn cond_test_sig(
    input: &str,
    signature: &Signature,
    bls_cache: Option<&mut BlsCache>,
    flags: u32,
) -> Result<(Allocator, SpendBundleConditions), ValidationErr> {
    cond_test_cb(input, flags, None, signature, bls_cache)
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP | HAS_RELATIVE_CONDITION);

    assert_eq!(spend.seconds_relative, Some(50));
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP | HAS_RELATIVE_CONDITION);

    assert_eq!(spend.seconds_relative, Some(50));
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
#[case(AGG_SIG_PARENT, "{pubkey} ({msg1}")]
#[case(AGG_SIG_PUZZLE, "{pubkey} ({msg1}")]
#[case(AGG_SIG_AMOUNT, "{pubkey} ({msg1}")]
#[case(AGG_SIG_PUZZLE_AMOUNT, "{pubkey} ({msg1}")]
#[case(AGG_SIG_PARENT_PUZZLE, "{pubkey} ({msg1}")]
#[case(AGG_SIG_PARENT_AMOUNT, "{pubkey} ({msg1}")]
#[case(ASSERT_CONCURRENT_SPEND, "{coin12}")]
#[case(ASSERT_CONCURRENT_PUZZLE, "{h2}")]
fn test_strict_args_count(
    #[case] condition: ConditionOpcode,
    #[case] arg: &str,
    #[values(STRICT_ARGS_COUNT, 0)] flags: u32,
) {
    // extra args are disallowed when STRICT_ARGS_COUNT is set
    let ret = cond_test_flag(
        &format!(
            "((({{h1}} ({{h2}} (123 ((({} ({} ( 1337 )))))",
            condition as u8, arg
        ),
        flags | DONT_VALIDATE_SIGNATURE,
    );
    if flags == 0 {
        // two of the cases won't pass, even when garbage at the end is allowed.
        if condition == ASSERT_COIN_ANNOUNCEMENT {
            assert_eq!(ret.unwrap_err().1, ErrorCode::AssertCoinAnnouncementFailed,);
        } else if condition == ASSERT_PUZZLE_ANNOUNCEMENT {
            assert_eq!(
                ret.unwrap_err().1,
                ErrorCode::AssertPuzzleAnnouncementFailed,
            );
        } else {
            assert!(ret.is_ok());
        }
    } else {
        assert_eq!(ret.unwrap_err().1, ErrorCode::InvalidCondition);
    }
}

#[cfg(test)]
#[rstest]
#[case(0x07, "0x1337", "({coin12}")]
#[case(0x04, "0x1337", "({h1}")]
#[case(0x06, "0x1337", "({h1} ({h2}")]
#[case(0x05, "0x1337", "({h1} (123")]
#[case(0x02, "0x1337", "({h2}")]
#[case(0x03, "0x1337", "({h2} (123")]
#[case(0x01, "0x1337", "(123")]
#[case(0, "0x1337", "")]
fn test_message_strict_args_count(
    #[values(0, 1)] pad: u8,
    #[case] mode: u8,
    #[case] msg: &str,
    #[case] arg: &str,
    #[values(STRICT_ARGS_COUNT, 0)] flags: u32,
) {
    // extra args are disallowed when STRICT_ARG_COUNT is set
    // pad determines whether the extra (unknown) argument is added to the
    // SEND_MESSAGE or the RECEIVE_MESSAGE condition
    let extra1 = if pad == 0 { "(1337" } else { "" };
    let extra2 = if pad == 1 { "(1337" } else { "" };
    let ret = cond_test_flag(
        &format!(
            "((({{h1}} ({{h2}} (123 (((66 ({mode} ({msg} {arg} {extra1} ) ((67 ({mode} ({msg} {extra2} ) ))))"
        ),
        flags | DONT_VALIDATE_SIGNATURE,
    );
    if flags == 0 {
        ret.unwrap();
    } else {
        assert_eq!(ret.unwrap_err().1, ErrorCode::InvalidCondition);
    }
}

#[cfg(test)]
#[rstest]
#[case(ASSERT_SECONDS_ABSOLUTE, "104", "", |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.seconds_absolute, 104))]
#[case(ASSERT_SECONDS_RELATIVE, "101", "", |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.seconds_relative, Some(101)))]
#[case(ASSERT_HEIGHT_RELATIVE, "101", "", |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.height_relative, Some(101)))]
#[case(ASSERT_HEIGHT_ABSOLUTE, "100", "", |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.height_absolute, 100))]
#[case(ASSERT_BEFORE_SECONDS_ABSOLUTE, "104", "", |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.before_seconds_absolute, Some(104)))]
#[case(ASSERT_BEFORE_SECONDS_RELATIVE, "101", "", |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.before_seconds_relative, Some(101)))]
#[case(ASSERT_BEFORE_HEIGHT_RELATIVE, "101", "", |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.before_height_relative, Some(101)))]
#[case(ASSERT_BEFORE_HEIGHT_ABSOLUTE, "100", "", |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.before_height_absolute, Some(100)))]
#[case(RESERVE_FEE, "100", "", |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.reserve_fee, 100))]
#[case(CREATE_COIN_ANNOUNCEMENT, "{msg1}", "((61 ({c11} )", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(ASSERT_COIN_ANNOUNCEMENT, "{c11}", "((60 ({msg1} )", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(CREATE_PUZZLE_ANNOUNCEMENT, "{msg1}", "((63 ({p21} )", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(ASSERT_PUZZLE_ANNOUNCEMENT, "{p21}", "((62 ({msg1} )", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(ASSERT_MY_AMOUNT, "123", "", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(ASSERT_MY_BIRTH_SECONDS, "123", "", |_: &SpendBundleConditions, s: &SpendConditions| { assert_eq!(s.birth_seconds, Some(123)); })]
#[case(ASSERT_MY_BIRTH_HEIGHT, "123", "", |_: &SpendBundleConditions, s: &SpendConditions| { assert_eq!(s.birth_height, Some(123)); })]
#[case(ASSERT_MY_COIN_ID, "{coin12}", "", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(ASSERT_MY_PARENT_ID, "{h1}", "", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(ASSERT_MY_PUZZLEHASH, "{h2}", "", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(ASSERT_CONCURRENT_SPEND, "{coin12}", "", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(ASSERT_CONCURRENT_PUZZLE, "{h2}", "", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(AGG_SIG_PARENT, "{pubkey} ({msg1}", "", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(AGG_SIG_PUZZLE, "{pubkey} ({msg1}", "", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(AGG_SIG_AMOUNT, "{pubkey} ({msg1}", "", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(AGG_SIG_PUZZLE_AMOUNT, "{pubkey} ({msg1}", "", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(AGG_SIG_PARENT_PUZZLE, "{pubkey} ({msg1}", "", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(AGG_SIG_PARENT_AMOUNT, "{pubkey} ({msg1}", "", |_: &SpendBundleConditions, _: &SpendConditions| {})]
fn test_extra_arg(
    #[case] condition: ConditionOpcode,
    #[case] arg: &str,
    #[case] extra_cond: &str,
    #[case] test: impl Fn(&SpendBundleConditions, &SpendConditions),
) {
    let signature = match condition {
        AGG_SIG_PARENT
        | AGG_SIG_PUZZLE
        | AGG_SIG_AMOUNT
        | AGG_SIG_PUZZLE_AMOUNT
        | AGG_SIG_PARENT_PUZZLE
        | AGG_SIG_PARENT_AMOUNT => sign_tx(H1, H2, 123, condition, MSG1),
        _ => Signature::default(),
    };

    // extra args are ignored in consensus mode
    // and a failure in mempool mode
    assert_eq!(
        cond_test_sig(
            &format!(
                "((({{h1}} ({{h2}} (123 ((({} ({} ( 1337 ) {} ))))",
                condition as u8, arg, extra_cond
            ),
            &signature,
            None,
            MEMPOOL_MODE,
        )
        .unwrap_err()
        .1,
        ErrorCode::InvalidCondition
    );

    let (a, conds) = cond_test_sig(
        &format!(
            "((({{h1}} ({{h2}} (123 ((({} ({} ( 1337 ) {} ))))",
            condition as u8, arg, extra_cond
        ),
        &signature,
        None,
        0,
    )
    .unwrap();

    let has_agg_sig = [
        AGG_SIG_PARENT,
        AGG_SIG_PUZZLE,
        AGG_SIG_AMOUNT,
        AGG_SIG_PUZZLE_AMOUNT,
        AGG_SIG_PARENT_PUZZLE,
        AGG_SIG_PARENT_AMOUNT,
    ]
    .contains(&condition);

    let expected_cost = if has_agg_sig { 1_200_000 } else { 0 };

    let expected_flags = if has_agg_sig { 0 } else { ELIGIBLE_FOR_DEDUP };

    assert_eq!(conds.cost, expected_cost);
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!((spend.flags & ELIGIBLE_FOR_DEDUP), expected_flags);

    test(&conds, spend);
}

#[cfg(test)]
#[rstest]
#[case(ASSERT_SECONDS_ABSOLUTE, "104", |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.seconds_absolute, 104))]
#[case(ASSERT_SECONDS_ABSOLUTE, "0", |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.seconds_absolute, 0))]
#[case(ASSERT_SECONDS_ABSOLUTE, "-1", |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.seconds_absolute, 0))]
#[case(ASSERT_SECONDS_RELATIVE, "101", |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.seconds_relative, Some(101)))]
#[case(ASSERT_SECONDS_RELATIVE, "0", |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.seconds_relative, Some(0)))]
#[case(ASSERT_SECONDS_RELATIVE, "-1", |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.seconds_relative, None))]
#[case(ASSERT_HEIGHT_RELATIVE, "101", |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.height_relative, Some(101)))]
#[case(ASSERT_HEIGHT_RELATIVE, "0", |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.height_relative, Some(0)))]
#[case(ASSERT_HEIGHT_RELATIVE, "-1", |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.height_relative, None))]
#[case(ASSERT_HEIGHT_ABSOLUTE, "100", |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.height_absolute, 100))]
#[case(ASSERT_HEIGHT_ABSOLUTE, "-1", |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.height_absolute, 0))]
#[case(ASSERT_BEFORE_SECONDS_ABSOLUTE, "104", |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.before_seconds_absolute, Some(104)))]
#[case(ASSERT_BEFORE_SECONDS_RELATIVE, "101", |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.before_seconds_relative, Some(101)))]
#[case(ASSERT_BEFORE_SECONDS_RELATIVE, "0", |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.before_seconds_relative, Some(0)))]
#[case(ASSERT_BEFORE_HEIGHT_RELATIVE, "101", |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.before_height_relative, Some(101)))]
#[case(ASSERT_BEFORE_HEIGHT_RELATIVE, "0", |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.before_height_relative, Some(0)))]
#[case(ASSERT_BEFORE_HEIGHT_ABSOLUTE, "100", |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.before_height_absolute, Some(100)))]
#[case(RESERVE_FEE, "100", |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.reserve_fee, 100))]
#[case(ASSERT_MY_AMOUNT, "123", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(ASSERT_MY_BIRTH_SECONDS, "123", |_: &SpendBundleConditions, s: &SpendConditions| { assert_eq!(s.birth_seconds, Some(123)); })]
#[case(ASSERT_MY_BIRTH_HEIGHT, "123", |_: &SpendBundleConditions, s: &SpendConditions| { assert_eq!(s.birth_height, Some(123)); })]
#[case(ASSERT_MY_COIN_ID, "{coin12}", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(ASSERT_MY_PARENT_ID, "{h1}", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(ASSERT_MY_PUZZLEHASH, "{h2}", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(ASSERT_CONCURRENT_SPEND, "{coin12}", |_: &SpendBundleConditions, _: &SpendConditions| {})]
#[case(ASSERT_CONCURRENT_PUZZLE, "{h2}", |_: &SpendBundleConditions, _: &SpendConditions| {})]
fn test_single_condition(
    #[case] condition: ConditionOpcode,
    #[case] arg: &str,
    #[case] test: impl Fn(&SpendBundleConditions, &SpendConditions),
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert!((spend.flags & ELIGIBLE_FOR_DEDUP) != 0);

    test(&conds, spend);
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
    assert_eq!(spend.seconds_relative, None);
}

#[cfg(test)]
#[rstest]
#[case(
    ASSERT_SECONDS_ABSOLUTE,
    "0x010000000000000000",
    ErrorCode::AssertSecondsAbsoluteFailed
)]
#[case(
    ASSERT_SECONDS_RELATIVE,
    "0x010000000000000000",
    ErrorCode::AssertSecondsRelativeFailed
)]
#[case(
    ASSERT_HEIGHT_ABSOLUTE,
    "0x0100000000",
    ErrorCode::AssertHeightAbsoluteFailed
)]
#[case(
    ASSERT_HEIGHT_RELATIVE,
    "0x0100000000",
    ErrorCode::AssertHeightRelativeFailed
)]
#[case(
    ASSERT_BEFORE_SECONDS_ABSOLUTE,
    "-1",
    ErrorCode::AssertBeforeSecondsAbsoluteFailed
)]
#[case(
    ASSERT_BEFORE_SECONDS_ABSOLUTE,
    "0",
    ErrorCode::ImpossibleSecondsAbsoluteConstraints
)]
#[case(
    ASSERT_BEFORE_SECONDS_RELATIVE,
    "-1",
    ErrorCode::AssertBeforeSecondsRelativeFailed
)]
#[case(
    ASSERT_BEFORE_HEIGHT_ABSOLUTE,
    "-1",
    ErrorCode::AssertBeforeHeightAbsoluteFailed
)]
#[case(
    ASSERT_BEFORE_HEIGHT_ABSOLUTE,
    "0",
    ErrorCode::ImpossibleHeightAbsoluteConstraints
)]
#[case(
    ASSERT_BEFORE_HEIGHT_RELATIVE,
    "-1",
    ErrorCode::AssertBeforeHeightRelativeFailed
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

// this test includes multiple instances of the same condition, to ensure we
// aggregate the resulting condition correctly. The values we pass are:
// 100, 503, 90
#[cfg(test)]
#[rstest]
// we use the MAX value
#[case(ASSERT_SECONDS_ABSOLUTE, |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.seconds_absolute, 503))]
#[case(ASSERT_SECONDS_RELATIVE, |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.seconds_relative, Some(503)))]
#[case(ASSERT_HEIGHT_RELATIVE, |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.height_relative, Some(503)))]
#[case(ASSERT_HEIGHT_ABSOLUTE, |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.height_absolute, 503))]
// we use the SUM of the values
#[case(RESERVE_FEE, |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.reserve_fee, 693))]
// we use the MIN value
#[case(ASSERT_BEFORE_SECONDS_ABSOLUTE, |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.before_seconds_absolute, Some(90)))]
#[case(ASSERT_BEFORE_SECONDS_RELATIVE, |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.before_seconds_relative, Some(90)))]
#[case(ASSERT_BEFORE_HEIGHT_RELATIVE, |_: &SpendBundleConditions, s: &SpendConditions| assert_eq!(s.before_height_relative, Some(90)))]
#[case(ASSERT_BEFORE_HEIGHT_ABSOLUTE, |c: &SpendBundleConditions, _: &SpendConditions| assert_eq!(c.before_height_absolute, Some(90)))]
fn test_multiple_conditions(
    #[case] condition: ConditionOpcode,
    #[case] test: impl Fn(&SpendBundleConditions, &SpendConditions),
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert!((spend.flags & ELIGIBLE_FOR_DEDUP) != 0);

    test(&conds, spend);
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
#[case(AGG_SIG_PARENT)]
#[case(AGG_SIG_PUZZLE)]
#[case(AGG_SIG_AMOUNT)]
#[case(AGG_SIG_PUZZLE_AMOUNT)]
#[case(AGG_SIG_PARENT_PUZZLE)]
#[case(AGG_SIG_PARENT_AMOUNT)]
#[case(ASSERT_CONCURRENT_SPEND)]
#[case(ASSERT_CONCURRENT_PUZZLE)]
fn test_missing_arg(#[case] condition: ConditionOpcode) {
    // extra args are disallowed in mempool mode
    assert_eq!(
        cond_test_flag(
            &format!("((({{h1}} ({{h2}} (123 ((({} )))))", condition as u8),
            0
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP | HAS_RELATIVE_CONDITION);

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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
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
    assert_eq!(a.atom(conds.spends[0].puzzle_hash).as_ref(), H2);
    assert_eq!(*conds.spends[1].coin_id, test_coin_id(H2, H2, 123));
    assert_eq!(a.atom(conds.spends[1].puzzle_hash).as_ref(), H2);
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
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
    assert_eq!(a.atom(conds.spends[0].puzzle_hash).as_ref(), H2);
    assert_eq!(*conds.spends[1].coin_id, test_coin_id(H2, H2, 123));
    assert_eq!(a.atom(conds.spends[1].puzzle_hash).as_ref(), H2);
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_single_assert_my_puzzle_hash_mismatch() {
    // ASSERT_MY_PUZZLEHASH
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((72 ({h1} )))))")
            .unwrap_err()
            .1,
        ErrorCode::AssertMyPuzzleHashFailed
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
        ErrorCode::AssertMyPuzzleHashFailed
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert_eq!(c.puzzle_hash.as_ref(), H2);
        assert_eq!(c.amount, 42_u64);
        assert_eq!(c.hint, a.nil());
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
    assert_eq!(conds.removal_amount, 0xffff_ffff_ffff_ffff);
    assert_eq!(conds.addition_amount, 0xffff_ffff_ffff_ffff);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 0xffff_ffff_ffff_ffff));
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert_eq!(c.puzzle_hash.as_ref(), H2);
        assert_eq!(c.amount, 0xffff_ffff_ffff_ffff_u64);
        assert_eq!(c.hint, a.nil());
    }
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP | ELIGIBLE_FOR_FF);
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
        ErrorCode::CoinAmountExceedsMaximum
    );
}

#[test]
fn test_create_coin_negative_amount() {
    // CREATE_COIN
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((51 ({h2} (-1 )))))")
            .unwrap_err()
            .1,
        ErrorCode::CoinAmountNegative
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert!(c.puzzle_hash.as_ref() == H2);
        assert!(c.amount == 42_u64);
        assert!(a.atom(c.hint).as_ref() == H1.to_vec());
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert!(c.puzzle_hash.as_ref() == H2);
        assert!(c.amount == 42_u64);
        assert!(a.atom(c.hint).as_ref() == H1.to_vec());
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert!(c.puzzle_hash.as_ref() == H2);
        assert!(c.amount == 42_u64);
        assert!(a.atom(c.hint).as_ref() == H1.to_vec());
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert_eq!(c.puzzle_hash.as_ref(), H2);
        assert_eq!(c.amount, 42_u64);
        assert_eq!(c.hint, a.nil());
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert_eq!(c.puzzle_hash.as_ref(), H2);
        assert_eq!(c.amount, 42_u64);
        assert_eq!(c.hint, a.nil());
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.create_coin.len(), 1);

    for c in &spend.create_coin {
        assert!(c.puzzle_hash.as_ref() == H2);
        assert!(c.amount == 42_u64);
        assert!(a.atom(c.hint).as_ref() == MSG1.to_vec());
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.create_coin.len(), 1);

    for c in &spend.create_coin {
        assert_eq!(c.puzzle_hash.as_ref(), H2);
        assert_eq!(c.amount, 42_u64);
        assert_eq!(c.hint, a.nil());
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.create_coin.len(), 1);

    for c in &spend.create_coin {
        assert_eq!(c.puzzle_hash.as_ref(), H2);
        assert_eq!(c.amount, 42_u64);
        assert_eq!(a.atom(c.hint).as_ref(), H1.to_vec());
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.create_coin.len(), 1);
    for c in &spend.create_coin {
        assert_eq!(c.puzzle_hash.as_ref(), H2);
        assert_eq!(c.amount, 42_u64);
        assert_eq!(c.hint, a.nil());
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.create_coin.len(), 2);

    assert!(spend.create_coin.contains(&NewCoin {
        puzzle_hash: H2.into(),
        amount: 42_u64,
        hint: a.nil()
    }));
    assert!(spend.create_coin.contains(&NewCoin {
        puzzle_hash: H2.into(),
        amount: 43_u64,
        hint: a.nil()
    }));
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP | ELIGIBLE_FOR_FF);
}

#[test]
fn test_create_coin_exceed_cost() {
    // CREATE_COIN
    // ensure that we terminate parsing conditions once they exceed the max cost
    assert_eq!(
        cond_test_cb(
            "((({h1} ({h2} (123 ({} )))",
            0,
            Some(Box::new(|a: &mut Allocator| -> NodePtr {
                let mut rest: NodePtr = a.nil();

                for i in 0..6500 {
                    // this builds one CREATE_COIN condition
                    // borrow-rules prevent this from being succint
                    let coin = a.nil();
                    let val = a.new_atom(&u64_to_bytes(i)).unwrap();
                    let coin = a.new_pair(val, coin).unwrap();
                    let val = a.new_atom(H2).unwrap();
                    let coin = a.new_pair(val, coin).unwrap();
                    let val = a.new_atom(&u64_to_bytes(u64::from(CREATE_COIN))).unwrap();
                    let coin = a.new_pair(val, coin).unwrap();

                    // add the CREATE_COIN condition to the list (called rest)
                    rest = a.new_pair(coin, rest).unwrap();
                }
                rest
            })),
            &Signature::default(),
            None,
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
fn test_duplicate_create_coin_with_hint() {
    // CREATE_COIN
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((51 ({h2} (42 (({h1})) ((51 ({h2} (42 ) ))))")
            .unwrap_err()
            .1,
        ErrorCode::DuplicateOutput
    );
}

#[cfg(test)]
fn agg_sig_vec(c: ConditionOpcode, s: &SpendConditions) -> &[(PublicKey, NodePtr)] {
    match c {
        AGG_SIG_ME => &s.agg_sig_me,
        AGG_SIG_PARENT => &s.agg_sig_parent,
        AGG_SIG_PUZZLE => &s.agg_sig_puzzle,
        AGG_SIG_AMOUNT => &s.agg_sig_amount,
        AGG_SIG_PUZZLE_AMOUNT => &s.agg_sig_puzzle_amount,
        AGG_SIG_PARENT_AMOUNT => &s.agg_sig_parent_amount,
        AGG_SIG_PARENT_PUZZLE => &s.agg_sig_parent_puzzle,
        _ => {
            panic!("unexpected");
        }
    }
}

#[cfg(test)]
#[rstest]
#[case(AGG_SIG_ME)]
#[case(AGG_SIG_PARENT)]
#[case(AGG_SIG_PUZZLE)]
#[case(AGG_SIG_AMOUNT)]
#[case(AGG_SIG_PUZZLE_AMOUNT)]
#[case(AGG_SIG_PARENT_PUZZLE)]
#[case(AGG_SIG_PARENT_AMOUNT)]
fn test_single_agg_sig_me(
    #[case] condition: ConditionOpcode,
    #[values(MEMPOOL_MODE, 0)] mempool: u32,
) {
    let signature = sign_tx(H1, H2, 123, condition, MSG1);
    let (a, conds) = cond_test_sig(
        &format!("((({{h1}} ({{h2}} (123 ((({condition} ({{pubkey}} ({{msg1}} )))))"),
        &signature,
        None,
        mempool,
    )
    .unwrap();

    assert_eq!(conds.cost, AGG_SIG_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);

    let agg_sigs = agg_sig_vec(condition, spend);
    assert_eq!(agg_sigs.len(), 1);
    for c in agg_sigs {
        assert_eq!(c.0, PublicKey::from_bytes(PUBKEY).unwrap());
        assert_eq!(a.atom(c.1).as_ref(), MSG1);
    }
    assert_eq!(spend.flags, 0);
}

#[cfg(test)]
#[rstest]
#[case(AGG_SIG_ME)]
#[case(AGG_SIG_PARENT)]
#[case(AGG_SIG_PUZZLE)]
#[case(AGG_SIG_AMOUNT)]
#[case(AGG_SIG_PUZZLE_AMOUNT)]
#[case(AGG_SIG_PARENT_PUZZLE)]
#[case(AGG_SIG_PARENT_AMOUNT)]
fn test_duplicate_agg_sig(
    #[case] condition: ConditionOpcode,
    #[values(MEMPOOL_MODE, 0)] mempool: u32,
) {
    // we cannot deduplicate AGG_SIG conditions. Their signatures will be
    // aggregated, and so must all copies of the public keys
    let (a, conds) =
        cond_test_flag(&format!("((({{h1}} ({{h2}} (123 ((({} ({{pubkey}} ({{msg1}} ) (({} ({{pubkey}} ({{msg1}} ) ))))", condition as u8, condition as u8),
            mempool | DONT_VALIDATE_SIGNATURE)
            .unwrap();

    assert_eq!(conds.cost, AGG_SIG_COST * 2);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);

    let agg_sigs = agg_sig_vec(condition, spend);
    assert_eq!(agg_sigs.len(), 2);
    for c in agg_sigs {
        assert_eq!(c.0, PublicKey::from_bytes(PUBKEY).unwrap());
        assert_eq!(a.atom(c.1).as_ref(), MSG1);
    }
    assert_eq!(spend.flags, 0);
}

#[cfg(test)]
#[rstest]
#[case(AGG_SIG_ME)]
#[case(AGG_SIG_PARENT)]
#[case(AGG_SIG_PUZZLE)]
#[case(AGG_SIG_AMOUNT)]
#[case(AGG_SIG_PUZZLE_AMOUNT)]
#[case(AGG_SIG_PARENT_PUZZLE)]
#[case(AGG_SIG_PARENT_AMOUNT)]
#[case(AGG_SIG_UNSAFE)]
fn test_agg_sig_invalid_pubkey(
    #[case] condition: ConditionOpcode,
    #[values(MEMPOOL_MODE, 0)] mempool: u32,
) {
    assert_eq!(
        cond_test_flag(
            &format!(
                "((({{h1}} ({{h2}} (123 ((({} ({{h2}} ({{msg1}} )))))",
                condition as u8
            ),
            mempool | DONT_VALIDATE_SIGNATURE
        )
        .unwrap_err()
        .1,
        ErrorCode::InvalidPublicKey
    );
}

#[cfg(test)]
#[rstest]
#[case(AGG_SIG_ME)]
#[case(AGG_SIG_PARENT)]
#[case(AGG_SIG_PUZZLE)]
#[case(AGG_SIG_AMOUNT)]
#[case(AGG_SIG_PUZZLE_AMOUNT)]
#[case(AGG_SIG_PARENT_PUZZLE)]
#[case(AGG_SIG_PARENT_AMOUNT)]
#[case(AGG_SIG_UNSAFE)]
fn test_agg_sig_infinity_pubkey(
    #[case] condition: ConditionOpcode,
    #[values(MEMPOOL_MODE, 0)] mempool: u32,
) {
    let ret = cond_test_flag(
        &format!(
            "((({{h1}} ({{h2}} (123 ((({} (0xc00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000 ({{msg1}} )))))",
            condition as u8
            ),
            mempool
    );

    assert_eq!(ret.unwrap_err().1, ErrorCode::InvalidPublicKey);
}

#[cfg(test)]
#[rstest]
#[case(AGG_SIG_ME)]
#[case(AGG_SIG_PARENT)]
#[case(AGG_SIG_PUZZLE)]
#[case(AGG_SIG_AMOUNT)]
#[case(AGG_SIG_PUZZLE_AMOUNT)]
#[case(AGG_SIG_PARENT_PUZZLE)]
#[case(AGG_SIG_PARENT_AMOUNT)]
fn test_agg_sig_invalid_msg(
    #[case] condition: ConditionOpcode,
    #[values(MEMPOOL_MODE, 0)] mempool: u32,
) {
    assert_eq!(
        cond_test_flag(
            &format!(
                "((({{h1}} ({{h2}} (123 ((({} ({{pubkey}} ({{longmsg}} )))))",
                condition as u8
            ),
            mempool
        )
        .unwrap_err()
        .1,
        ErrorCode::InvalidMessage
    );
}

#[cfg(test)]
#[rstest]
#[case(AGG_SIG_ME)]
#[case(AGG_SIG_PARENT)]
#[case(AGG_SIG_PUZZLE)]
#[case(AGG_SIG_AMOUNT)]
#[case(AGG_SIG_PUZZLE_AMOUNT)]
#[case(AGG_SIG_PARENT_PUZZLE)]
#[case(AGG_SIG_PARENT_AMOUNT)]
fn test_agg_sig_exceed_cost(#[case] condition: ConditionOpcode) {
    // ensure that we terminate parsing conditions once they exceed the max cost
    assert_eq!(
        cond_test_cb(
            "((({h1} ({h2} (123 ({} )))",
            0,
            Some(Box::new(move |a: &mut Allocator| -> NodePtr {
                let mut rest: NodePtr = a.nil();

                for _i in 0..9167 {
                    // this builds one AGG_SIG_* condition
                    // borrow-rules prevent this from being succint
                    let aggsig = a.nil();
                    let val = a.new_atom(MSG1).unwrap();
                    let aggsig = a.new_pair(val, aggsig).unwrap();
                    let val = a.new_atom(PUBKEY).unwrap();
                    let aggsig = a.new_pair(val, aggsig).unwrap();
                    let val = a.new_atom(&u64_to_bytes(u64::from(condition))).unwrap();
                    let aggsig = a.new_pair(val, aggsig).unwrap();

                    // add the condition to the list (called rest)
                    rest = a.new_pair(aggsig, rest).unwrap();
                }
                rest
            })),
            &Signature::default(),
            None,
        )
        .unwrap_err()
        .1,
        ErrorCode::CostExceeded
    );
}

#[test]
fn test_single_agg_sig_unsafe() {
    // AGG_SIG_UNSAFE
    let signature = sign_tx(H1, H2, 123, 49, MSG1);

    let (a, conds) = cond_test_sig(
        "((({h1} ({h2} (123 (((49 ({pubkey} ({msg1} )))))",
        &signature,
        None,
        0,
    )
    .unwrap();

    assert_eq!(conds.cost, AGG_SIG_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(conds.agg_sig_unsafe.len(), 1);
    for (pk, msg) in &conds.agg_sig_unsafe {
        assert_eq!(*pk, PublicKey::from_bytes(PUBKEY).unwrap());
        assert_eq!(a.atom(*msg).as_ref(), MSG1);
    }
    assert_eq!(spend.flags, 0);
}

#[cfg(test)]
#[rstest]
#[case(AGG_SIG_ME)]
#[case(AGG_SIG_UNSAFE)]
#[case(AGG_SIG_PARENT)]
#[case(AGG_SIG_PUZZLE)]
#[case(AGG_SIG_AMOUNT)]
#[case(AGG_SIG_PUZZLE_AMOUNT)]
#[case(AGG_SIG_PARENT_PUZZLE)]
#[case(AGG_SIG_PARENT_AMOUNT)]
fn test_agg_sig_extra_arg(#[case] condition: ConditionOpcode) {
    // extra args are ignored in consensus mode
    let (a, conds) = cond_test_flag(
        &format!(
            "((({{h1}} ({{h2}} (123 ((({} ({{pubkey}} ({{msg1}} ( 1337 ) ))))",
            condition as u8
        ),
        DONT_VALIDATE_SIGNATURE,
    )
    .unwrap();

    assert_eq!(conds.cost, 1_200_000);
    assert_eq!(conds.spends.len(), 1);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert!((spend.flags & ELIGIBLE_FOR_DEDUP) == 0);

    if condition != AGG_SIG_UNSAFE {
        let agg_sigs = agg_sig_vec(condition, spend);
        assert_eq!(agg_sigs.len(), 1);
    }

    // but not in mempool mode
    assert_eq!(
        cond_test_flag(
            &format!(
                "((({{h1}} ({{h2}} (123 ((({} ({{pubkey}} ({{msg1}} ( 1337 ) ))))",
                condition as u8
            ),
            MEMPOOL_MODE,
        )
        .unwrap_err()
        .1,
        ErrorCode::InvalidCondition
    );
}

#[test]
fn test_agg_sig_unsafe_invalid_terminator() {
    // AGG_SIG_UNSAFE
    // in non-mempool mode, even an invalid terminator is allowed
    let signature = sign_tx(H1, H2, 123, 49, MSG1);
    let (a, conds) = cond_test_sig(
        "((({h1} ({h2} (123 (((49 ({pubkey} ({msg1} 456 ))))",
        &signature,
        None,
        0,
    )
    .unwrap();

    assert_eq!(conds.cost, AGG_SIG_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(conds.agg_sig_unsafe.len(), 1);
    for (pk, msg) in &conds.agg_sig_unsafe {
        assert_eq!(*pk, PublicKey::from_bytes(PUBKEY).unwrap());
        assert_eq!(a.atom(*msg).as_ref(), MSG1);
    }
    assert_eq!(spend.flags, 0);
}

#[test]
fn test_agg_sig_me_invalid_terminator() {
    // AGG_SIG_ME
    // this has an invalid list terminator of the argument list. This is OK
    // according to the original consensus rules
    let signature = sign_tx(H1, H2, 123, 50, MSG1);
    let (a, conds) = cond_test_sig(
        "((({h1} ({h2} (123 (((50 ({pubkey} ({msg1} 456 ))))",
        &signature,
        None,
        0,
    )
    .unwrap();

    assert_eq!(conds.cost, AGG_SIG_COST);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.agg_sig_me.len(), 1);
    for (pk, msg) in &conds.agg_sig_unsafe {
        assert_eq!(*pk, PublicKey::from_bytes(PUBKEY).unwrap());
        assert_eq!(a.atom(*msg).as_ref(), MSG1);
    }
    assert_eq!(spend.flags, 0);
}

#[test]
fn test_duplicate_agg_sig_unsafe() {
    // AGG_SIG_UNSAFE
    // these conditions may not be deduplicated
    let mut signature = sign_tx(H1, H2, 123, 49, MSG1);
    signature.aggregate(&sign_tx(H1, H2, 123, 49, MSG1));
    let (a, conds) = cond_test_sig(
        "((({h1} ({h2} (123 (((49 ({pubkey} ({msg1} ) ((49 ({pubkey} ({msg1} ) ))))",
        &signature,
        None,
        0,
    )
    .unwrap();

    assert_eq!(conds.cost, AGG_SIG_COST * 2);
    assert_eq!(conds.spends.len(), 1);
    assert_eq!(conds.removal_amount, 123);
    assert_eq!(conds.addition_amount, 0);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(conds.agg_sig_unsafe.len(), 2);
    for (pk, msg) in &conds.agg_sig_unsafe {
        assert_eq!(*pk, PublicKey::from_bytes(PUBKEY).unwrap());
        assert_eq!(a.atom(*msg).as_ref(), MSG1);
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
        ErrorCode::InvalidPublicKey
    );
}

#[test]
fn test_agg_sig_unsafe_long_msg() {
    // AGG_SIG_UNSAFE
    assert_eq!(
        cond_test("((({h1} ({h2} (123 (((49 ({pubkey} ({longmsg} )))))")
            .unwrap_err()
            .1,
        ErrorCode::InvalidMessage
    );
}

#[cfg(test)]
fn final_message(
    parent: &[u8; 32],
    puzzle: &[u8; 32],
    amount: u64,
    opcode: u16,
    msg: &[u8],
) -> Vec<u8> {
    use crate::allocator::make_allocator;
    use crate::gen::make_aggsig_final_message::make_aggsig_final_message;
    use crate::gen::owned_conditions::OwnedSpendConditions;
    use chia_protocol::Coin;
    use clvmr::LIMIT_HEAP;

    let coin = Coin::new(Bytes32::from(parent), Bytes32::from(puzzle), amount);

    let mut a: Allocator = make_allocator(LIMIT_HEAP);
    let spend = SpendConditions::new(
        a.new_atom(parent.as_slice()).expect("should pass"),
        amount,
        a.new_atom(puzzle.as_slice()).expect("test should pass"),
        Arc::new(Bytes32::try_from(coin.coin_id()).expect("test should pass")),
    );

    let spend = OwnedSpendConditions::from(&a, spend);

    let mut final_msg = msg.to_vec();
    make_aggsig_final_message(opcode, &mut final_msg, &spend, &TEST_CONSTANTS);
    final_msg
}

#[cfg(test)]
fn sign_tx(
    parent: &[u8; 32],
    puzzle: &[u8; 32],
    amount: u64,
    opcode: u16,
    msg: &[u8],
) -> Signature {
    use chia_bls::{sign, SecretKey};

    let final_msg = final_message(parent, puzzle, amount, opcode, msg);
    sign(&SecretKey::from_bytes(SECRET_KEY).unwrap(), final_msg)
}

#[cfg(test)]
#[rstest]
// these are the suffixes used for AGG_SIG_* conditions (other than
// AGG_SIG_UNSAFE)
#[case("0xccd5bb71183532bff220ba46c268991a3ff07eb358e8255a65c30a2dce0e5fbb")]
#[case("0xbaf5d69c647c91966170302d18521b0a85663433d161e72c826ed08677b53a74")]
#[case("0x284fa2ef486c7a41cc29fc99c9d08376161e93dd37817edb8219f42dca7592c4")]
#[case("0xcda186a9cd030f7a130fae45005e81cae7a90e0fa205b75f6aebc0d598e0348e")]
#[case("0x0f7d90dff0613e6901e24dae59f1e690f18b8f5fbdcf1bb192ac9deaf7de22ad")]
#[case("0x585796bd90bb553c0430b87027ffee08d88aba0162c6e1abbbcc6b583f2ae7f9")]
#[case("0x2ebfdae17b29d83bae476a25ea06f0c4bd57298faddbbc3ec5ad29b9b86ce5df")]
// The same suffixes, but 1 byte prepended
#[case("0x01ccd5bb71183532bff220ba46c268991a3ff07eb358e8255a65c30a2dce0e5fbb")]
#[case("0x01baf5d69c647c91966170302d18521b0a85663433d161e72c826ed08677b53a74")]
#[case("0x01284fa2ef486c7a41cc29fc99c9d08376161e93dd37817edb8219f42dca7592c4")]
#[case("0x01cda186a9cd030f7a130fae45005e81cae7a90e0fa205b75f6aebc0d598e0348e")]
#[case("0x010f7d90dff0613e6901e24dae59f1e690f18b8f5fbdcf1bb192ac9deaf7de22ad")]
#[case("0x01585796bd90bb553c0430b87027ffee08d88aba0162c6e1abbbcc6b583f2ae7f9")]
#[case("0x012ebfdae17b29d83bae476a25ea06f0c4bd57298faddbbc3ec5ad29b9b86ce5df")]
fn test_agg_sig_unsafe_invalid_msg(
    #[case] msg: &str,
    #[values(43, 44, 45, 46, 47, 48, 49, 50)] opcode: u16,
) {
    let signature = sign_tx(
        H1,
        H2,
        123,
        opcode,
        &hex::decode(&msg[2..]).expect("msg not hex"),
    );

    let ret = cond_test_sig(
        format!("((({{h1}} ({{h2}} (123 ((({opcode} ({{pubkey}} ({msg} )))))").as_str(),
        &signature,
        None,
        0,
    );
    if opcode == AGG_SIG_UNSAFE {
        assert_eq!(ret.unwrap_err().1, ErrorCode::InvalidMessage);
    } else {
        assert!(ret.is_ok());
    }
}

#[test]
fn test_agg_sig_unsafe_exceed_cost() {
    // AGG_SIG_UNSAFE
    // ensure that we terminate parsing conditions once they exceed the max cost
    assert_eq!(
        cond_test_cb(
            "((({h1} ({h2} (123 ({} )))",
            0,
            Some(Box::new(|a: &mut Allocator| -> NodePtr {
                let mut rest: NodePtr = a.nil();

                for _i in 0..9167 {
                    // this builds one AGG_SIG_UNSAFE condition
                    // borrow-rules prevent this from being succint
                    let aggsig = a.nil();
                    let val = a.new_atom(MSG1).unwrap();
                    let aggsig = a.new_pair(val, aggsig).unwrap();
                    let val = a.new_atom(PUBKEY).unwrap();
                    let aggsig = a.new_pair(val, aggsig).unwrap();
                    let val = a
                        .new_atom(&u64_to_bytes(u64::from(AGG_SIG_UNSAFE)))
                        .unwrap();
                    let aggsig = a.new_pair(val, aggsig).unwrap();

                    // add the AGG_SIG_UNSAFE condition to the list (called rest)
                    rest = a.new_pair(aggsig, rest).unwrap();
                }
                rest
            })),
            &Signature::default(),
            None,
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
fn test_remark() {
    // REMARK
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.agg_sig_me.len(), 0);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_remark_with_arg() {
    // REMARK, but with one unknown argument
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
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
        assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
        assert_eq!(spend.agg_sig_me.len(), 0);
        assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);

        let spend = &conds.spends[1];
        assert_eq!(*spend.coin_id, test_coin_id(H2, H2, 123));
        assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
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
        assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H1);
        assert_eq!(spend.agg_sig_me.len(), 0);
        assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);

        let spend = &conds.spends[1];
        assert_eq!(*spend.coin_id, test_coin_id(H2, H2, 123));
        assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
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
        assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H1);
        assert_eq!(spend.agg_sig_me.len(), 0);
        assert!((spend.flags & ELIGIBLE_FOR_DEDUP) != 0);
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
        assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H1);
        assert_eq!(spend.agg_sig_me.len(), 0);
        assert!((spend.flags & ELIGIBLE_FOR_DEDUP) != 0);

        let spend = &conds.spends[1];
        assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
        assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
        assert_eq!(spend.agg_sig_me.len(), 0);
        assert!((spend.flags & ELIGIBLE_FOR_DEDUP) != 0);
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
#[case(ASSERT_MY_BIRTH_HEIGHT, |s: &SpendConditions| assert_eq!(s.birth_height, Some(100)))]
#[case(ASSERT_MY_BIRTH_SECONDS, |s: &SpendConditions| assert_eq!(s.birth_seconds, Some(100)))]
fn test_multiple_my_birth_assertions(
    #[case] condition: ConditionOpcode,
    #[case] test: impl Fn(&SpendConditions),
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
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert!((spend.flags & ELIGIBLE_FOR_DEDUP) != 0);

    test(spend);
}

#[test]
fn test_assert_ephemeral() {
    // ASSERT_EPHEMERAL
    // the coin11 value is the coinID computed from (H1, H1, 123).
    // coin11 is the first coin we spend in this case.
    // 51 is CREATE_COIN, 76 is ASSERT_EPHEMERAL
    let test = "(\
       (({h1} ({h1} (123 (\
           ((51 ({h2} (123 ) \
           ))\
       (({coin11} ({h2} (123 (\
           ((76 ) \
           ))\
       ))";
    // we don't expect any error
    let (a, conds) = cond_test(test).unwrap();

    // our spends don't add any additional constraints
    assert_eq!(conds.agg_sig_unsafe.len(), 0);
    assert_eq!(conds.reserve_fee, 0);
    assert_eq!(conds.cost, CREATE_COIN_COST);
    // we spend a coin worth 123, into a new coin worth 123
    // then we spend that coin burning the value. i.e. we spend 123 * 2 and only
    // add 123. the net is a removal of 123
    assert_eq!(conds.removal_amount, 246);
    assert_eq!(conds.addition_amount, 123);

    assert_eq!(conds.spends.len(), 2);
    let spend = &conds.spends[0];
    assert_eq!(*spend.coin_id, test_coin_id(H1, H1, 123));
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H1);
    assert_eq!(spend.agg_sig_me.len(), 0);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);

    let spend = &conds.spends[1];
    assert_eq!(
        *spend.coin_id,
        test_coin_id((&(*conds.spends[0].coin_id)).into(), H2, 123)
    );
    assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
    assert_eq!(spend.agg_sig_me.len(), 0);
    assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);
}

#[test]
fn test_assert_ephemeral_wrong_ph() {
    // ASSERT_EPHEMERAL
    // the coin11 value is the coinID computed from (H1, H1, 123). The first
    // coin we spend in this case
    // 51 is CREATE_COIN, 76 is ASSERT_EPHEMERAL
    // in this case the puzzle hash doesn't match the coin the parent paid to,
    // so it's not a valid spend
    let test = "(\
       (({h1} ({h1} (123 (\
           ((51 ({h2} (123 ) \
           ))\
       (({coin11} ({h1} (123 (\
           ((76 ) \
           ))\
       ))";

    // this is an invalid ASSERT_EPHEMERAL
    assert_eq!(
        cond_test(test).unwrap_err().1,
        ErrorCode::AssertEphemeralFailed
    );
}

#[test]
fn test_assert_ephemeral_wrong_amount() {
    // ASSERT_EPHEMERAL
    // the coin11 value is the coinID computed from (H1, H1, 123). The first
    // coin we spend in this case
    // 51 is CREATE_COIN, 76 is ASSERT_EPHEMERAL
    // in this case the amount doesn't match the coin the parent paid to,
    // so it's not a valid spend
    let test = "(\
       (({h1} ({h1} (123 (\
           ((51 ({h2} (123 ) \
           ))\
       (({coin11} ({h2} (122 (\
           ((76 ) \
           ))\
       ))";

    // this is an invalid ASSERT_EPHEMERAL
    assert_eq!(
        cond_test(test).unwrap_err().1,
        ErrorCode::AssertEphemeralFailed
    );
}

#[test]
fn test_assert_ephemeral_wrong_parent() {
    // ASSERT_EPHEMERAL
    // the coin12 value is the coinID computed from (H1, H2, 123). This is *not*
    // the coin we spend first
    // 51 is CREATE_COIN, 76 is ASSERT_EPHEMERAL
    // in this case the amount doesn't match the coin the parent paid to,
    // so it's not a valid spend
    let test = "(\
       (({h1} ({h1} (123 (\
           ((51 ({h2} (123 ) \
           ))\
       (({coin12} ({h2} (123 (\
           ((76 ) \
           ))\
       ))";

    // this is an invalid ASSERT_EPHEMERAL
    assert_eq!(
        cond_test(test).unwrap_err().1,
        ErrorCode::AssertEphemeralFailed
    );
}

#[cfg(test)]
#[rstest]
// the default expected errors are post soft-fork, when both new rules are
// activated
#[case(ASSERT_HEIGHT_ABSOLUTE, None)]
#[case(ASSERT_HEIGHT_RELATIVE, Some(ErrorCode::EphemeralRelativeCondition))]
#[case(ASSERT_SECONDS_ABSOLUTE, None)]
#[case(ASSERT_SECONDS_RELATIVE, Some(ErrorCode::EphemeralRelativeCondition))]
#[case(ASSERT_MY_BIRTH_HEIGHT, Some(ErrorCode::EphemeralRelativeCondition))]
#[case(ASSERT_MY_BIRTH_SECONDS, Some(ErrorCode::EphemeralRelativeCondition))]
#[case(ASSERT_BEFORE_HEIGHT_ABSOLUTE, None)]
#[case(
    ASSERT_BEFORE_HEIGHT_RELATIVE,
    Some(ErrorCode::EphemeralRelativeCondition)
)]
#[case(ASSERT_BEFORE_SECONDS_ABSOLUTE, None)]
#[case(
    ASSERT_BEFORE_SECONDS_RELATIVE,
    Some(ErrorCode::EphemeralRelativeCondition)
)]
fn test_relative_condition_on_ephemeral(
    #[case] condition: ConditionOpcode,
    #[case] expect_error: Option<ErrorCode>,
) {
    // this test ensures that we disallow relative conditions (including
    // assert-my-birth conditions) on ephemeral coin spends.
    // We run these test cases for every combination of enabling/disabling
    // assert-before conditions as well as disallowing relative conditions on
    // ephemeral coins

    let cond = condition as u8;

    // the coin11 value is the coinID computed from (H1, H1, 123).
    // coin11 is the first coin we spend in this case.
    // 51 is CREATE_COIN
    let test = format!(
        "(\
       (({{h1}} ({{h1}} (123 (\
           ((51 ({{h2}} (123 ) \
           ))\
       (({{coin11}} ({{h2}} (123 (\
           (({cond} (1000 ) \
           ))\
       ))"
    );

    if let Some(err) = expect_error {
        assert_eq!(cond_test(&test).unwrap_err().1, err);
    } else {
        // we don't expect any error
        let (a, conds) = cond_test(&test).unwrap();

        assert_eq!(conds.reserve_fee, 0);
        assert_eq!(conds.cost, CREATE_COIN_COST);

        assert_eq!(conds.spends.len(), 2);
        let spend = &conds.spends[0];
        assert_eq!(*spend.coin_id, test_coin_id(H1, H1, 123));
        assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H1);
        assert_eq!(spend.agg_sig_me.len(), 0);
        assert_eq!(spend.flags, ELIGIBLE_FOR_DEDUP);

        let spend = &conds.spends[1];
        assert_eq!(
            *spend.coin_id,
            test_coin_id((&(*conds.spends[0].coin_id)).into(), H2, 123)
        );
        assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
        assert_eq!(spend.agg_sig_me.len(), 0);
        assert!((spend.flags & ELIGIBLE_FOR_DEDUP) != 0);
    }
}

#[cfg(test)]
#[rstest]
// the scale factor is 10000
#[case("((90 (1 )", 10000)]
// 0 is OK (but does it make sense?)
#[case("((90 (0 )", 0)]
// the cost accumulates
#[case("((90 (1 ) ((90 (2 ) ((90 (3 )", (1 + 2 + 3) * 10000)]
// the cost can be large
#[case("((90 (10000 )", 100_000_000)]
// the upper cost limit in the test is 11000000000
#[case("((90 (1100000 )", 11_000_000_000)]
// additional arguments are ignored
#[case("((90 (1 ( 42 ( 1337 )", 10000)]
// reserved opcodes with fixed cost
#[case("((256 )", 100)]
#[case("((257 )", 106)]
#[case("((258 )", 112)]
#[case("((259 )", 119)]
#[case("((260 )", 127)]
#[case("((261 )", 135)]
#[case("((262 )", 143)]
#[case("((263 )", 152)]
#[case("((264 )", 162)]
#[case("((265 )", 172)]
#[case("((266 )", 183)]
#[case("((504 )", 338_000_000)]
#[case("((505 )", 359_000_000)]
#[case("((506 )", 382_000_000)]
#[case("((507 )", 406_000_000)]
#[case("((508 )", 431_000_000)]
#[case("((509 )", 458_000_000)]
#[case("((510 )", 487_000_000)]
#[case("((511 )", 517_000_000)]
#[case("((512 )", 100)]
#[case("((513 )", 106)]
#[case("((0xff00 )", 100)]
#[case("((0xff01 )", 106)]
fn test_softfork_condition(#[case] conditions: &str, #[case] expected_cost: Cost) {
    // SOFTFORK (90)
    let (_, spends) =
        cond_test_flag(&format!("((({{h1}} ({{h2}} (1234 ({conditions}))))"), 0).unwrap();
    assert_eq!(spends.cost, expected_cost);
}

#[cfg(test)]
#[rstest]
// the cost argument must be positive
#[case("((90 (-1 )", ErrorCode::InvalidSoftforkCost)]
// the cost argument may not exceed 2^32-1
#[case("((90 (0x0100000000 )", ErrorCode::InvalidSoftforkCost)]
// the test has a cost limit of 11000000000
#[case("((90 (0x00ffffffff )", ErrorCode::CostExceeded)]
#[case("((90 )", ErrorCode::InvalidCondition)]
fn test_softfork_condition_failures(#[case] conditions: &str, #[case] expected_err: ErrorCode) {
    // SOFTFORK (90)
    assert_eq!(
        cond_test_flag(&format!("((({{h1}} ({{h2}} (1234 ({conditions}))))"), 0)
            .unwrap_err()
            .1,
        expected_err
    );
}

#[cfg(test)]
#[rstest]
#[case(CREATE_PUZZLE_ANNOUNCEMENT, 1000, None)]
#[case(
    CREATE_PUZZLE_ANNOUNCEMENT,
    1025,
    Some(ErrorCode::TooManyAnnouncements)
)]
#[case(
    ASSERT_PUZZLE_ANNOUNCEMENT,
    1024,
    Some(ErrorCode::AssertPuzzleAnnouncementFailed)
)]
#[case(
    ASSERT_PUZZLE_ANNOUNCEMENT,
    1025,
    Some(ErrorCode::TooManyAnnouncements)
)]
#[case(CREATE_COIN_ANNOUNCEMENT, 1000, None)]
#[case(CREATE_COIN_ANNOUNCEMENT, 1025, Some(ErrorCode::TooManyAnnouncements))]
#[case(
    ASSERT_COIN_ANNOUNCEMENT,
    1024,
    Some(ErrorCode::AssertCoinAnnouncementFailed)
)]
#[case(ASSERT_COIN_ANNOUNCEMENT, 1025, Some(ErrorCode::TooManyAnnouncements))]
#[case(
    ASSERT_CONCURRENT_SPEND,
    1024,
    Some(ErrorCode::AssertConcurrentSpendFailed)
)]
#[case(ASSERT_CONCURRENT_SPEND, 1025, Some(ErrorCode::TooManyAnnouncements))]
#[case(
    ASSERT_CONCURRENT_PUZZLE,
    1024,
    Some(ErrorCode::AssertConcurrentPuzzleFailed)
)]
#[case(ASSERT_CONCURRENT_PUZZLE, 1025, Some(ErrorCode::TooManyAnnouncements))]
fn test_limit_announcements(
    #[case] cond: ConditionOpcode,
    #[case] count: i32,
    #[case] expect_err: Option<ErrorCode>,
) {
    let r = cond_test_cb(
        "((({h1} ({h1} (123 ({} )))",
        0,
        Some(Box::new(move |a: &mut Allocator| -> NodePtr {
            let mut rest: NodePtr = a.nil();

            // generate a lot of announcements
            for _ in 0..count {
                // this builds one condition
                // borrow-rules prevent this from being succint
                let ann = a.nil();
                let val = a.new_atom(H2).unwrap();
                let ann = a.new_pair(val, ann).unwrap();
                let val = a.new_atom(&u64_to_bytes(u64::from(cond))).unwrap();
                let ann = a.new_pair(val, ann).unwrap();

                // add the condition to the list
                rest = a.new_pair(ann, rest).unwrap();
            }
            rest
        })),
        &Signature::default(),
        None,
    );

    if expect_err.is_some() {
        assert_eq!(r.unwrap_err().1, expect_err.unwrap());
    } else {
        r.unwrap();
    }
}

#[test]
fn test_eligible_for_ff_assert_parent() {
    // this is a model example of a spend that's eligible for FF
    // it mimics the output of singleton_top_layer_v1_1
    // the ASSERT_MY_PARENT_ID is only allowed as the second condition
    // 73=ASSERT_MY_AMOUNT
    // 71=ASSERT_MY_PARENT_ID
    // 51=CREATE_COIN
    let test = "(\
       (({h1} ({h2} (123 (\
           ((73 (123 ) \
           ((71 ({h1} ) \
           ((51 ({h2} (123 ) \
           ))\
       ))";

    let (_a, cond) = cond_test_flag(test, DONT_VALIDATE_SIGNATURE).expect("cond_test");
    assert!(cond.spends.len() == 1);
    assert!((cond.spends[0].flags & ELIGIBLE_FOR_FF) != 0);
}

#[test]
fn test_eligible_for_ff_even_amount() {
    // coins with even amounts cannot be singletons, even if all other
    // conditions are met
    // 73=ASSERT_MY_AMOUNT
    // 71=ASSERT_MY_PARENT_ID
    // 51=CREATE_COIN
    let test = "(\
       (({h1} ({h2} (122 (\
           ((73 (122 ) \
           ((71 ({h1} ) \
           ((51 ({h2} (122 ) \
           ))\
       ))";

    let (_a, cond) = cond_test(test).expect("cond_test");
    assert!(cond.spends.len() == 1);
    assert!((cond.spends[0].flags & ELIGIBLE_FOR_FF) == 0);
}

#[cfg(test)]
#[rstest]
#[case(123, "{h2}", true)]
#[case(121, "{h2}", true)]
#[case(122, "{h1}", false)]
#[case(1, "{h1}", false)]
#[case(123, "{h1}", false)]
fn test_eligible_for_ff_output_coin(#[case] amount: u64, #[case] ph: &str, #[case] eligible: bool) {
    // in order to be elgibible for fast forward, there needs to be an output
    // coin with the same puzzle hash
    // 51=CREATE_COIN
    let test: &str = &format!(
        "(\
       (({{h1}} ({{h2}} (123 (\
           ((51 ({ph} ({amount} ) \
           ))\
       ))"
    );

    let (_a, cond) = cond_test(test).expect("cond_test");
    assert!(cond.spends.len() == 1);
    let flags = cond.spends[0].flags;
    if eligible {
        assert!((flags & ELIGIBLE_FOR_FF) != 0);
    } else {
        assert!((flags & ELIGIBLE_FOR_FF) == 0);
    }
}

#[cfg(test)]
#[rstest]
#[case(ASSERT_MY_PARENT_ID, "{h1}")]
#[case(ASSERT_MY_COIN_ID, "{coin12}")]
fn test_eligible_for_ff_invalid_assert_parent(
    #[case] condition: ConditionOpcode,
    #[case] arg: &str,
) {
    // the ASSERT_MY_PARENT_ID is only allowed as the second condition
    // and ASSERT_MY_COIN_ID is disallowed
    // 73=ASSERT_MY_AMOUNT
    // 51=CREATE_COIN
    let test: &str = &format!(
        "(\
       (({{h1}} ({{h2}} (123 (\
           (({condition} ({arg} ) \
           ((73 (123 ) \
           ((51 ({{h2}} (123 ) \
           ))\
       ))"
    );

    let (_a, cond) = cond_test(test).expect("cond_test");
    assert!(cond.spends.len() == 1);
    assert!((cond.spends[0].flags & ELIGIBLE_FOR_FF) == 0);
}

#[cfg(test)]
#[rstest]
#[case(AGG_SIG_ME, false)]
#[case(AGG_SIG_PARENT, false)]
#[case(AGG_SIG_PARENT_AMOUNT, false)]
#[case(AGG_SIG_PARENT_PUZZLE, false)]
#[case(AGG_SIG_UNSAFE, true)]
#[case(AGG_SIG_PUZZLE, true)]
#[case(AGG_SIG_AMOUNT, true)]
#[case(AGG_SIG_PUZZLE_AMOUNT, true)]
fn test_eligible_for_ff_invalid_agg_sig_me(
    #[case] condition: ConditionOpcode,
    #[case] eligible: bool,
) {
    let signature = sign_tx(H1, H2, 1, condition, MSG1);

    // 51=CREATE_COIN
    let test: &str = &format!(
        "(\
       (({{h1}} ({{h2}} (1 (\
           (({condition} ({{pubkey}} ({{msg1}} ) \
           ((51 ({{h2}} (1 ) \
           ))\
       ))"
    );

    let (_a, cond) = cond_test_sig(test, &signature, None, 0).expect("cond_test");
    assert!(cond.spends.len() == 1);
    let flags = cond.spends[0].flags;
    if eligible {
        assert!((flags & ELIGIBLE_FOR_FF) != 0);
    } else {
        assert!((flags & ELIGIBLE_FOR_FF) == 0);
    }
}

// test aggregate signature validation. Both positive and negative cases

#[cfg(test)]
fn add_signature(sig: &mut Signature, puzzle: &mut String, opcode: ConditionOpcode) {
    if opcode == 0 {
        return;
    }
    sig.aggregate(&sign_tx(H1, H2, 123, opcode, MSG1));
    puzzle.push_str(format!("(({opcode} ({{pubkey}} ({{msg1}} )").as_str());
}

#[cfg(test)]
fn populate_cache(opcode: ConditionOpcode, bls_cache: &mut BlsCache) {
    use chia_bls::hash_to_g2;
    let msg = final_message(H1, H2, 123, opcode, MSG1);
    // Otherwise, we need to calculate the pairing and add it to the cache.
    let mut aug_msg = PUBKEY.to_vec();
    aug_msg.extend_from_slice(msg.as_ref());
    let aug_hash = hash_to_g2(&aug_msg);

    let gt = aug_hash.pair(&PublicKey::from_bytes(PUBKEY).unwrap());
    bls_cache.update(&aug_msg, gt);
}

#[cfg(test)]
#[rstest]
fn test_agg_sig(
    #[values(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15)] a: u32,
    #[values(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15)] b: u32,
    #[values(true, false)] expect_pass: bool,
    #[values(true, false)] with_cache: bool,
) {
    use chia_bls::{sign, SecretKey};
    let mut signature = Signature::default();
    let mut bls_cache = BlsCache::default();
    let cache: Option<&mut BlsCache> = if with_cache {
        populate_cache(43, &mut bls_cache);
        populate_cache(44, &mut bls_cache);
        populate_cache(45, &mut bls_cache);
        populate_cache(46, &mut bls_cache);
        populate_cache(47, &mut bls_cache);
        populate_cache(48, &mut bls_cache);
        populate_cache(49, &mut bls_cache);
        populate_cache(50, &mut bls_cache);
        Some(&mut bls_cache)
    } else {
        None
    };

    let combination = (a << 4) | b;
    let mut puzzle: String = "((({h1} ({h2} (123 (".into();
    let opcodes: &[ConditionOpcode] = &[
        AGG_SIG_PARENT,
        AGG_SIG_PUZZLE,
        AGG_SIG_AMOUNT,
        AGG_SIG_PUZZLE_AMOUNT,
        AGG_SIG_PARENT_AMOUNT,
        AGG_SIG_PARENT_PUZZLE,
        AGG_SIG_UNSAFE,
        AGG_SIG_ME,
    ];
    for (i, opcode) in opcodes.iter().enumerate() {
        if (combination & (1 << i)) == 0 {
            continue;
        }
        add_signature(&mut signature, &mut puzzle, *opcode);
    }
    puzzle.push_str("))))");
    if !expect_pass {
        signature.aggregate(&sign(
            &SecretKey::from_bytes(SECRET_KEY).unwrap(),
            b"foobar",
        ));
    }
    assert_eq!(
        expect_pass,
        cond_test_sig(puzzle.as_str(), &signature, cache, 0).is_ok()
    );
}

// the message condition takes a mode-parameter. This is a 6-bit integer that
// determines which aspects of the sending spend and receiving spends must
// match. The second argument is the message. The message and mode must always
// match between the sender and receiver
// Additional parameters depend on the mode. If the mode specifies 0
// committments, there are no additional parameters (however, this is not
// allowed in mempool mode).
// The mode integer is a bitfield, the 3 least significant bits indicate
// properties of the destination spend, the 3 most significant bits indicate
// properties of the sending spend.
// 0b100000 = sender parent id
// 0b010000 = sender puzzle hash
// 0b001000 = sender amount
// 0b000100 = receiver parent id
// 0b000010 = receiver puzzle hash
// 0b000001 = receiver amount

// 66=SEND_MESSAGE
// 67=RECEIVE_MESSAGE
#[cfg(test)]
enum Ex {
    Fail,
    Pass,
}

#[cfg(test)]
#[rstest]
// no committment
#[case("(66 (0 ({msg1} ) ((67 (0 ({msg1} )", Ex::Pass)]
#[case("(66 (0 ({msg2} ) ((67 (0 ({msg1} )", Ex::Fail)]
#[case("(66 (0 ({msg1} ) ((67 (0 ({msg2} )", Ex::Fail)]
// only sender coin-ID committment
#[case("(66 (0x38 ({msg1} ) ((67 (0x38 ({msg1} ({coin12} )", Ex::Pass)]
#[case("(66 (0x38 ({msg1} ) ((67 (0x38 ({msg2} ({coin12} )", Ex::Fail)]
#[case("(66 (0x38 ({msg2} ) ((67 (0x38 ({msg1} ({coin12} )", Ex::Fail)]
#[case("(66 (0x38 ({msg1} ) ((67 (0x38 ({msg1} ({coin21} )", Ex::Fail)]
// only receiver coin-ID committment
#[case("(66 (0x07 ({msg1} ({coin12} ) ((67 (0x07 ({msg1} )", Ex::Pass)]
#[case("(66 (0x07 ({msg1} ({coin21} ) ((67 (0x07 ({msg1} )", Ex::Fail)]
#[case("(66 (0x07 ({msg2} ({coin12} ) ((67 (0x07 ({msg1} )", Ex::Fail)]
#[case("(66 (0x07 ({msg1} ({coin12} ) ((67 (0x07 ({msg2} )", Ex::Fail)]
// only sender parent committment
#[case("(66 (0x20 ({msg1} ) ((67 (0x20 ({msg1} ({h1} )", Ex::Pass)]
#[case("(66 (0x20 ({msg1} ) ((67 (0x20 ({msg1} ({h2} )", Ex::Fail)]
#[case("(66 (0x20 ({msg2} ) ((67 (0x20 ({msg1} ({h1} )", Ex::Fail)]
#[case("(66 (0x20 ({msg1} ) ((67 (0x20 ({msg2} ({h1} )", Ex::Fail)]
// only receiver parent committment
#[case("(66 (0x04 ({msg1} ({h1} ) ((67 (0x04 ({msg1} )", Ex::Pass)]
#[case("(66 (0x04 ({msg1} ({h2} ) ((67 (0x04 ({msg1} )", Ex::Fail)]
#[case("(66 (0x04 ({msg2} ({h1} ) ((67 (0x04 ({msg1} )", Ex::Fail)]
#[case("(66 (0x04 ({msg1} ({h1} ) ((67 (0x04 ({msg2} )", Ex::Fail)]
// only sender puzzle committment
#[case("(66 (0x10 ({msg1} ) ((67 (0x10 ({msg1} ({h2} )", Ex::Pass)]
#[case("(66 (0x10 ({msg1} ) ((67 (0x10 ({msg1} ({h1} )", Ex::Fail)]
#[case("(66 (0x10 ({msg2} ) ((67 (0x10 ({msg1} ({h2} )", Ex::Fail)]
#[case("(66 (0x10 ({msg1} ) ((67 (0x10 ({msg2} ({h2} )", Ex::Fail)]
// only receiver puzzle committment
#[case("(66 (0x02 ({msg1} ({h2} ) ((67 (0x02 ({msg1} )", Ex::Pass)]
#[case("(66 (0x02 ({msg1} ({h1} ) ((67 (0x02 ({msg1} )", Ex::Fail)]
#[case("(66 (0x02 ({msg2} ({h2} ) ((67 (0x02 ({msg1} )", Ex::Fail)]
#[case("(66 (0x02 ({msg1} ({h2} ) ((67 (0x02 ({msg2} )", Ex::Fail)]
// only sender amount committment
#[case("(66 (0x08 ({msg1} ) ((67 (0x08 ({msg1} (123 )", Ex::Pass)]
#[case("(66 (0x08 ({msg1} ) ((67 (0x08 ({msg1} (124 )", Ex::Fail)]
#[case("(66 (0x08 ({msg2} ) ((67 (0x08 ({msg1} (123 )", Ex::Fail)]
#[case("(66 (0x08 ({msg1} ) ((67 (0x08 ({msg2} (123 )", Ex::Fail)]
// only receiver amount committment
#[case("(66 (0x01 ({msg1} (123 ) ((67 (0x01 ({msg1} )", Ex::Pass)]
#[case("(66 (0x01 ({msg1} (124 ) ((67 (0x01 ({msg1} )", Ex::Fail)]
#[case("(66 (0x01 ({msg2} (123 ) ((67 (0x01 ({msg1} )", Ex::Fail)]
#[case("(66 (0x01 ({msg1} (123 ) ((67 (0x01 ({msg2} )", Ex::Fail)]
// only amount committment
#[case("(66 (0x09 ({msg1} (123 ) ((67 (0x09 ({msg1} (123 )", Ex::Pass)]
#[case("(66 (0x09 ({msg1} (124 ) ((67 (0x09 ({msg1} (123 )", Ex::Fail)]
#[case("(66 (0x09 ({msg1} (123 ) ((67 (0x09 ({msg1} (124 )", Ex::Fail)]
#[case("(66 (0x09 ({msg2} (123 ) ((67 (0x09 ({msg1} (123 )", Ex::Fail)]
#[case("(66 (0x09 ({msg1} (123 ) ((67 (0x09 ({msg2} (123 )", Ex::Fail)]
// only amount committment on receiver
#[case("(66 (0x39 ({msg1} (123 ) ((67 (0x39 ({msg1} ({coin12} )", Ex::Pass)]
#[case("(66 (0x39 ({msg1} (124 ) ((67 (0x39 ({msg1} ({coin12} )", Ex::Fail)]
#[case("(66 (0x39 ({msg1} (123 ) ((67 (0x39 ({msg1} ({coin21} )", Ex::Fail)]
#[case("(66 (0x39 ({msg2} (123 ) ((67 (0x39 ({msg1} ({coin12} )", Ex::Fail)]
#[case("(66 (0x39 ({msg1} (123 ) ((67 (0x39 ({msg2} ({coin12} )", Ex::Fail)]
// only amount committment on sender
#[case("(66 (0x0f ({msg1} ({coin12} ) ((67 (0x0f ({msg1} (123 )", Ex::Pass)]
#[case("(66 (0x0f ({msg1} ({coin12} ) ((67 (0x0f ({msg1} (124 )", Ex::Fail)]
#[case("(66 (0x0f ({msg1} ({coin21} ) ((67 (0x0f ({msg1} (123 )", Ex::Fail)]
#[case("(66 (0x0f ({msg2} ({coin12} ) ((67 (0x0f ({msg1} (123 )", Ex::Fail)]
#[case("(66 (0x0f ({msg1} ({coin12} ) ((67 (0x0f ({msg2} (123 )", Ex::Fail)]
// sender and receiver coin ID committment (full committment)
#[case(
    "(66 (0x3f ({msg1} ({coin12} ) ((67 (0x3f ({msg1} ({coin12} )",
    Ex::Pass
)]
#[case(
    "(67 (0x3f ({msg1} ({coin12} ) ((66 (0x3f ({msg1} ({coin12} )",
    Ex::Pass
)]
//   wrong coin-id
#[case(
    "(66 (0x3f ({msg1} ({coin21} ) ((67 (0x3f ({msg1} ({coin12} )",
    Ex::Fail
)]
#[case(
    "(66 (0x3f ({msg1} ({coin12} ) ((67 (0x3f ({msg1} ({coin21} )",
    Ex::Fail
)]
//   wrong message
#[case(
    "(66 (0x3f ({msg2} ({coin12} ) ((67 (0x3f ({msg1} ({coin12} )",
    Ex::Fail
)]
#[case(
    "(66 (0x3f ({msg1} ({coin12} ) ((67 (0x3f ({msg2} ({coin12} )",
    Ex::Fail
)]
//    mismatching message
#[case(
    "(67 (0x3f ({msg2} ({coin12} ) ((66 (0x3f ({msg1} ({coin12} )",
    Ex::Fail
)]
// sender and receiver puzzle committment
#[case("(66 (0x12 ({msg1} ({h2} ) ((67 (0x12 ({msg1} ({h2} )", Ex::Pass)]
#[case("(67 (0x12 ({msg1} ({h2} ) ((66 (0x12 ({msg1} ({h2} )", Ex::Pass)]
//    wrong messages
#[case("(66 (0x12 ({msg2} ({h2} ) ((67 (0x12 ({msg1} ({h2} )", Ex::Fail)]
#[case("(66 (0x12 ({msg1} ({h2} ) ((67 (0x12 ({msg2} ({h2} )", Ex::Fail)]
//    wrong puzzle
#[case("(66 (0x12 ({msg1} ({h1} ) ((67 (0x12 ({msg1} ({h2} )", Ex::Fail)]
#[case("(66 (0x12 ({msg1} ({h2} ) ((67 (0x12 ({msg1} ({h1} )", Ex::Fail)]
// sender parent and receiver puzzle committment
#[case("(66 (0x22 ({msg1} ({h2} ) ((67 (0x22 ({msg1} ({h1} )", Ex::Pass)]
#[case("(67 (0x22 ({msg1} ({h1} ) ((66 (0x22 ({msg1} ({h2} )", Ex::Pass)]
//    wrong messages
#[case("(66 (0x22 ({msg2} ({h2} ) ((67 (0x22 ({msg1} ({h1} )", Ex::Fail)]
#[case("(66 (0x22 ({msg1} ({h2} ) ((67 (0x22 ({msg2} ({h1} )", Ex::Fail)]
//    wrong puzzle
#[case("(66 (0x22 ({msg1} ({h1} ) ((67 (0x22 ({msg1} ({h1} )", Ex::Fail)]
//    wrong parent
#[case("(66 (0x22 ({msg1} ({h2} ) ((67 (0x22 ({msg1} ({h2} )", Ex::Fail)]
// sender parent and receiver puzzle & amount committment
#[case("(66 (0x23 ({msg1} ({h2} (123 ) ((67 (0x23 ({msg1} ({h1} )", Ex::Pass)]
#[case("(67 (0x23 ({msg1} ({h1} ) ((66 (0x23 ({msg1} ({h2} (123 )", Ex::Pass)]
//    wrong messages
#[case("(66 (0x23 ({msg2} ({h2} (123 ) ((67 (0x23 ({msg1} ({h1} )", Ex::Fail)]
#[case("(66 (0x23 ({msg1} ({h2} (123 ) ((67 (0x23 ({msg2} ({h1} )", Ex::Fail)]
//    wrong puzzle
#[case("(66 (0x23 ({msg1} ({h1} (123 ) ((67 (0x23 ({msg1} ({h1} )", Ex::Fail)]
//    wrong amount
#[case("(66 (0x23 ({msg1} ({h2} (122 ) ((67 (0x23 ({msg1} ({h1} )", Ex::Fail)]
//    wrong parent
#[case("(66 (0x23 ({msg1} ({h2} (123 ) ((67 (0x23 ({msg1} ({h2} )", Ex::Fail)]
// sender parent & puzzle and receiver puzzle committment
#[case("(66 (0x32 ({msg1} ({h2} ) ((67 (0x32 ({msg1} ({h1} ({h2} )", Ex::Pass)]
#[case("(67 (0x32 ({msg1} ({h1} ({h2} ) ((66 (0x32 ({msg1} ({h2} )", Ex::Pass)]
//    wrong messages
#[case("(66 (0x32 ({msg2} ({h2} ) ((67 (0x32 ({msg1} ({h1} ({h2} )", Ex::Fail)]
#[case("(66 (0x32 ({msg1} ({h2} ) ((67 (0x32 ({msg2} ({h1} ({h2} )", Ex::Fail)]
//    wrong puzzle
#[case("(66 (0x32 ({msg1} ({h1} ) ((67 (0x32 ({msg1} ({h1} ({h2} )", Ex::Fail)]
//    wrong puzzle
#[case("(66 (0x32 ({msg1} ({h2} ) ((67 (0x32 ({msg1} ({h1} ({h1} )", Ex::Fail)]
//    wrong parent
#[case("(66 (0x32 ({msg1} ({h2} ) ((67 (0x32 ({msg1} ({h2} ({h2} )", Ex::Fail)]
// No sender or no recipient of the message
#[case("(67 (0x12 ({msg1} ({coin12} )", Ex::Fail)]
#[case("(66 (0x12 ({msg1} ({coin12} )", Ex::Fail)]
fn test_message_conditions_single_spend(#[case] test_case: &str, #[case] expect: Ex) {
    let flags = MEMPOOL_MODE;
    let ret = cond_test_flag(&format!("((({{h1}} ({{h2}} (123 (({test_case}))))"), flags);

    let expect_pass = match expect {
        Ex::Pass => true,
        Ex::Fail => false,
    };

    if let Ok((a, conds)) = ret {
        assert!(expect_pass);
        assert_eq!(conds.cost, 0);
        assert_eq!(conds.spends.len(), 1);
        let spend = &conds.spends[0];
        assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
        assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
        assert_eq!(spend.flags, 0);
    } else if expect_pass {
        panic!("failed: {:?}", ret.unwrap_err().1);
    } else {
        let actual_err = ret.unwrap_err().1;
        println!("Error: {actual_err:?}");
        assert_eq!(ErrorCode::MessageNotSentOrReceived, actual_err);
    }
}

#[cfg(test)]
#[rstest]
#[case(512, None)]
#[case(513, Some(ErrorCode::TooManyAnnouncements))]
fn test_limit_messages(#[case] count: i32, #[case] expect_err: Option<ErrorCode>) {
    let r = cond_test_cb(
        "((({h1} ({h1} (123 ({} )))",
        0,
        Some(Box::new(move |a: &mut Allocator| -> NodePtr {
            let mut rest: NodePtr = a.nil();

            // generate a lot of announcements
            // this builds one condition
            // borrow-rules prevent this from being succint
            // (66 0x3f {msg1} {coin12})
            let send = a.nil();
            let val = a.new_atom(&test_coin_id(H1, H1, 123)).unwrap();
            let send = a.new_pair(val, send).unwrap();
            let val = a.new_atom(MSG1).unwrap();
            let send = a.new_pair(val, send).unwrap();
            let val = a.new_small_number(0x3f).unwrap();
            let send = a.new_pair(val, send).unwrap();
            let val = a.new_small_number(SEND_MESSAGE.into()).unwrap();
            let send = a.new_pair(val, send).unwrap();

            // (67 0x3f {msg1} {coin12})
            let recv = a.nil();
            let val = a.new_atom(&test_coin_id(H1, H1, 123)).unwrap();
            let recv = a.new_pair(val, recv).unwrap();
            let val = a.new_atom(MSG1).unwrap();
            let recv = a.new_pair(val, recv).unwrap();
            let val = a.new_small_number(0x3f).unwrap();
            let recv = a.new_pair(val, recv).unwrap();
            let val = a.new_small_number(RECEIVE_MESSAGE.into()).unwrap();
            let recv = a.new_pair(val, recv).unwrap();

            for _ in 0..count {
                // add the condition to the list
                rest = a.new_pair(send, rest).unwrap();
                rest = a.new_pair(recv, rest).unwrap();
            }
            rest
        })),
        &Signature::default(),
        None,
    );

    if expect_err.is_some() {
        assert_eq!(r.unwrap_err().1, expect_err.unwrap());
    } else {
        r.unwrap();
    }
}

#[cfg(test)]
#[rstest]
#[case("(66 (0x38 ({longmsg} )", ErrorCode::InvalidMessage)]
#[case("(66 (0x3c ({long} ({msg1} )", ErrorCode::InvalidParentId)]
#[case("(66 (0x3c ({msg2} ({msg1} )", ErrorCode::InvalidParentId)]
#[case("(66 (0x3a ({long} ({msg1} )", ErrorCode::InvalidPuzzleHash)]
#[case("(66 (0x3a ({msg2} ({msg1} )", ErrorCode::InvalidPuzzleHash)]
#[case("(66 (0x3f ({long} ({msg1} )", ErrorCode::InvalidCoinId)]
#[case("(66 (0x3f ({msg2} ({msg1} )", ErrorCode::InvalidCoinId)]
#[case(
    "(66 (0x08 ({msg1} ) ((67 (0x08 ({msg1} (-1 )",
    ErrorCode::CoinAmountNegative
)]
#[case(
    "(66 (0x08 ({msg1} ) ((67 (0x08 ({msg1} )",
    ErrorCode::InvalidCondition
)]
#[case(
    "(66 (0x01 ({msg1} (-1 ) ((67 (0x01 ({msg1} )",
    ErrorCode::CoinAmountNegative
)]
#[case(
    "(66 (0x01 ({msg1} ) ((67 (0x01 ({msg1} )",
    ErrorCode::InvalidCondition
)]
#[case(
    "(66 (0x02 ({msg1} ({msg2} ) ((67 (0x02 ({msg1} )",
    ErrorCode::InvalidPuzzleHash
)]
#[case(
    "(66 (0x02 ({msg1} ) ((67 (0x02 ({msg1} )",
    ErrorCode::InvalidCondition
)]
#[case(
    "(66 (0x10 ({msg1} ) ((67 (0x10 ({msg1} ({msg2} )",
    ErrorCode::InvalidPuzzleHash
)]
#[case(
    "(66 (0x10 ({msg1} ) ((67 (0x10 ({msg1} )",
    ErrorCode::InvalidCondition
)]
#[case(
    "(66 (0x04 ({msg1} ({msg2} ) ((67 (0x04 ({msg1} )",
    ErrorCode::InvalidParentId
)]
#[case(
    "(66 (0x04 ({msg1} ) ((67 (0x04 ({msg1} )",
    ErrorCode::InvalidCondition
)]
#[case(
    "(66 (0x20 ({msg1} ) ((67 (0x20 ({msg1} ({msg2} )",
    ErrorCode::InvalidParentId
)]
#[case(
    "(66 (0x20 ({msg1} ) ((67 (0x20 ({msg1} )",
    ErrorCode::InvalidCondition
)]
#[case(
    "(66 (0x07 ({msg1} ({msg2} ) ((67 (0x07 ({msg1} )",
    ErrorCode::InvalidCoinId
)]
#[case(
    "(66 (0x07 ({msg1} ) ((67 (0x07 ({msg1} )",
    ErrorCode::InvalidCondition
)]
#[case(
    "(66 (0x38 ({msg1} ) ((67 (0x38 ({msg1} ({msg2} )",
    ErrorCode::InvalidCoinId
)]
#[case(
    "(66 (0x38 ({msg1} ) ((67 (0x38 ({msg1} )",
    ErrorCode::InvalidCondition
)]
// message mode must be specified in canonical mode
#[case(
    "(66 (0x00 ({msg1} ) ((67 (0x00 ({msg1} )",
    ErrorCode::InvalidMessageMode
)]
#[case(
    "(66 (0x01 ({msg1} (123 ) ((67 (0x00 ({msg1} )",
    ErrorCode::InvalidMessageMode
)]
// negative messages modes are not allowed
#[case(
    "(66 (-1 ({msg1} (123 ) ((67 (0x01 ({msg1} )",
    ErrorCode::InvalidMessageMode
)]
#[case(
    "(66 (0x01 ({msg1} (123 ) ((67 (-1 ({msg1} )",
    ErrorCode::InvalidMessageMode
)]
// amounts must be specified in canonical mode
#[case(
    "(66 (0x01 ({msg1} (0x0040 ) ((67 (0x01 ({msg1} (123 )",
    ErrorCode::InvalidCoinAmount
)]
#[case(
    "(66 (0x01 ({msg1} (0x00 ) ((67 (0x01 ({msg1} (123 )",
    ErrorCode::InvalidCoinAmount
)]
// coin amounts can't be negative
#[case(
    "(66 (0x01 ({msg1} (-1 ) ((67 (0x01 ({msg1} (123 )",
    ErrorCode::CoinAmountNegative
)]
#[case(
    "(66 (0x01 ({msg1} (-1 ) ((67 (0x01 ({msg1} (123 )",
    ErrorCode::CoinAmountNegative
)]
fn test_message_conditions_failures(#[case] test_case: &str, #[case] expect: ErrorCode) {
    let flags = MEMPOOL_MODE;
    let ret = cond_test_flag(&format!("((({{h1}} ({{h2}} (123 (({test_case}))))"), flags);

    let Err(ValidationErr(_, code)) = ret else {
        panic!("expected failure: {expect:?}");
    };
    assert_eq!(code, expect);
}

#[cfg(test)]
#[rstest]
// no committment
#[case("(66 (0 ({msg1} )", "(67 (0 ({msg1} )", Ex::Pass)]
#[case("(66 (0 ({msg2} )", "(67 (0 ({msg1} )", Ex::Fail)]
#[case("(66 (0 ({msg1} )", "(67 (0 ({msg2} )", Ex::Fail)]
// only sender coin-ID committment
#[case("(66 (0x38 ({msg1} )", "(67 (0x38 ({msg1} ({coin12} )", Ex::Pass)]
#[case("(66 (0x38 ({msg1} )", "(67 (0x38 ({msg2} ({coin12} )", Ex::Fail)]
#[case("(66 (0x38 ({msg2} )", "(67 (0x38 ({msg1} ({coin12} )", Ex::Fail)]
#[case("(66 (0x38 ({msg1} )", "(67 (0x38 ({msg1} ({coin21} )", Ex::Fail)]
// only receiver coin-ID committment
#[case("(66 (0x07 ({msg1} ({coin21} )", "(67 (0x07 ({msg1} )", Ex::Pass)]
#[case("(66 (0x07 ({msg1} ({coin12} )", "(67 (0x07 ({msg1} )", Ex::Fail)]
#[case("(66 (0x07 ({msg2} ({coin21} )", "(67 (0x07 ({msg1} )", Ex::Fail)]
#[case("(66 (0x07 ({msg1} ({coin21} )", "(67 (0x07 ({msg2} )", Ex::Fail)]
// only sender parent committment
#[case("(66 (0x20 ({msg1} )", "(67 (0x20 ({msg1} ({h1} )", Ex::Pass)]
#[case("(66 (0x20 ({msg1} )", "(67 (0x20 ({msg1} ({h2} )", Ex::Fail)]
#[case("(66 (0x20 ({msg2} )", "(67 (0x20 ({msg1} ({h1} )", Ex::Fail)]
#[case("(66 (0x20 ({msg1} )", "(67 (0x20 ({msg2} ({h1} )", Ex::Fail)]
// only receiver parent committment
#[case("(66 (0x04 ({msg1} ({h2} )", "(67 (0x04 ({msg1} )", Ex::Pass)]
#[case("(66 (0x04 ({msg1} ({h1} )", "(67 (0x04 ({msg1} )", Ex::Fail)]
#[case("(66 (0x04 ({msg2} ({h2} )", "(67 (0x04 ({msg1} )", Ex::Fail)]
#[case("(66 (0x04 ({msg1} ({h2} )", "(67 (0x04 ({msg2} )", Ex::Fail)]
// only sender puzzle committment
#[case("(66 (0x10 ({msg1} )", "(67 (0x10 ({msg1} ({h2} )", Ex::Pass)]
#[case("(66 (0x10 ({msg1} )", "(67 (0x10 ({msg1} ({h1} )", Ex::Fail)]
#[case("(66 (0x10 ({msg2} )", "(67 (0x10 ({msg1} ({h2} )", Ex::Fail)]
#[case("(66 (0x10 ({msg1} )", "(67 (0x10 ({msg2} ({h2} )", Ex::Fail)]
// only receiver puzzle committment
#[case("(66 (0x02 ({msg1} ({h1} )", "(67 (0x02 ({msg1} )", Ex::Pass)]
#[case("(66 (0x02 ({msg1} ({h2} )", "(67 (0x02 ({msg1} )", Ex::Fail)]
#[case("(66 (0x02 ({msg2} ({h1} )", "(67 (0x02 ({msg1} )", Ex::Fail)]
#[case("(66 (0x02 ({msg1} ({h1} )", "(67 (0x02 ({msg2} )", Ex::Fail)]
// only sender amount committment
#[case("(66 (0x08 ({msg1} )", "(67 (0x08 ({msg1} (123 )", Ex::Pass)]
#[case("(66 (0x08 ({msg1} )", "(67 (0x08 ({msg1} (124 )", Ex::Fail)]
#[case("(66 (0x08 ({msg2} )", "(67 (0x08 ({msg1} (123 )", Ex::Fail)]
#[case("(66 (0x08 ({msg1} )", "(67 (0x08 ({msg2} (123 )", Ex::Fail)]
// only receiver amount committment
#[case("(66 (0x01 ({msg1} (123 )", "(67 (0x01 ({msg1} )", Ex::Pass)]
#[case("(66 (0x01 ({msg1} (124 )", "(67 (0x01 ({msg1} )", Ex::Fail)]
#[case("(66 (0x01 ({msg2} (123 )", "(67 (0x01 ({msg1} )", Ex::Fail)]
#[case("(66 (0x01 ({msg1} (123 )", "(67 (0x01 ({msg2} )", Ex::Fail)]
// only amount committment
#[case("(66 (0x09 ({msg1} (123 )", "(67 (0x09 ({msg1} (123 )", Ex::Pass)]
#[case("(66 (0x09 ({msg1} (124 )", "(67 (0x09 ({msg1} (123 )", Ex::Fail)]
#[case("(66 (0x09 ({msg1} (123 )", "(67 (0x09 ({msg1} (124 )", Ex::Fail)]
#[case("(66 (0x09 ({msg2} (123 )", "(67 (0x09 ({msg1} (123 )", Ex::Fail)]
#[case("(66 (0x09 ({msg1} (123 )", "(67 (0x09 ({msg2} (123 )", Ex::Fail)]
// only amount committment on receiver
#[case("(66 (0x39 ({msg1} (123 )", "(67 (0x39 ({msg1} ({coin12} )", Ex::Pass)]
#[case("(66 (0x39 ({msg1} (124 )", "(67 (0x39 ({msg1} ({coin12} )", Ex::Fail)]
#[case("(66 (0x39 ({msg1} (123 )", "(67 (0x39 ({msg1} ({coin21} )", Ex::Fail)]
#[case("(66 (0x39 ({msg2} (123 )", "(67 (0x39 ({msg1} ({coin12} )", Ex::Fail)]
#[case("(66 (0x39 ({msg1} (123 )", "(67 (0x39 ({msg2} ({coin12} )", Ex::Fail)]
// only amount committment on sender
#[case("(66 (0x0f ({msg1} ({coin21} )", "(67 (0x0f ({msg1} (123 )", Ex::Pass)]
#[case("(66 (0x0f ({msg1} ({coin21} )", "(67 (0x0f ({msg1} (124 )", Ex::Fail)]
#[case("(66 (0x0f ({msg1} ({coin12} )", "(67 (0x0f ({msg1} (123 )", Ex::Fail)]
#[case("(66 (0x0f ({msg2} ({coin21} )", "(67 (0x0f ({msg1} (123 )", Ex::Fail)]
#[case("(66 (0x0f ({msg1} ({coin21} )", "(67 (0x0f ({msg2} (123 )", Ex::Fail)]
// sender and receiver coin ID committment (full committment)
#[case(
    "(66 (0x3f ({msg1} ({coin21} )",
    "(67 (0x3f ({msg1} ({coin12} )",
    Ex::Pass
)]
#[case(
    "(67 (0x3f ({msg1} ({coin21} )",
    "(66 (0x3f ({msg1} ({coin12} )",
    Ex::Pass
)]
//   wrong coin-id
#[case(
    "(66 (0x3f ({msg1} ({coin12} )",
    "(67 (0x3f ({msg1} ({coin12} )",
    Ex::Fail
)]
#[case(
    "(66 (0x3f ({msg1} ({coin21} )",
    "(67 (0x3f ({msg1} ({coin21} )",
    Ex::Fail
)]
//   wrong message
#[case(
    "(66 (0x3f ({msg2} ({coin21} )",
    "(67 (0x3f ({msg1} ({coin12} )",
    Ex::Fail
)]
#[case(
    "(66 (0x3f ({msg1} ({coin21} )",
    "(67 (0x3f ({msg2} ({coin12} )",
    Ex::Fail
)]
//    mismatching message
#[case(
    "(67 (0x3f ({msg2} ({coin21} )",
    "(66 (0x3f ({msg1} ({coin12} )",
    Ex::Fail
)]
// sender and receiver puzzle committment
#[case("(66 (0x12 ({msg1} ({h1} )", "(67 (0x12 ({msg1} ({h2} )", Ex::Pass)]
#[case("(67 (0x12 ({msg1} ({h1} )", "(66 (0x12 ({msg1} ({h2} )", Ex::Pass)]
//    wrong messages
#[case("(66 (0x12 ({msg2} ({h1} )", "(67 (0x12 ({msg1} ({h2} )", Ex::Fail)]
#[case("(66 (0x12 ({msg1} ({h1} )", "(67 (0x12 ({msg2} ({h2} )", Ex::Fail)]
//    wrong puzzle
#[case("(66 (0x12 ({msg1} ({h2} )", "(67 (0x12 ({msg1} ({h2} )", Ex::Fail)]
#[case("(66 (0x12 ({msg1} ({h1} )", "(67 (0x12 ({msg1} ({h1} )", Ex::Fail)]
// sender parent and receiver puzzle committment
#[case("(66 (0x22 ({msg1} ({h1} )", "(67 (0x22 ({msg1} ({h1} )", Ex::Pass)]
#[case("(67 (0x22 ({msg1} ({h2} )", "(66 (0x22 ({msg1} ({h2} )", Ex::Pass)]
//    wrong messages
#[case("(66 (0x22 ({msg2} ({h1} )", "(67 (0x22 ({msg1} ({h1} )", Ex::Fail)]
#[case("(66 (0x22 ({msg1} ({h1} )", "(67 (0x22 ({msg2} ({h1} )", Ex::Fail)]
//    wrong puzzle
#[case("(66 (0x22 ({msg1} ({h2} )", "(67 (0x22 ({msg1} ({h1} )", Ex::Fail)]
//    wrong parent
#[case("(66 (0x22 ({msg1} ({h1} )", "(67 (0x22 ({msg1} ({h2} )", Ex::Fail)]
// sender parent and receiver puzzle & amount committment
#[case(
    "(66 (0x23 ({msg1} ({h1} (123 )",
    "(67 (0x23 ({msg1} ({h1} )",
    Ex::Pass
)]
#[case(
    "(67 (0x23 ({msg1} ({h2} )",
    "(66 (0x23 ({msg1} ({h2} (123 )",
    Ex::Pass
)]
//    wrong messages
#[case(
    "(66 (0x23 ({msg2} ({h1} (123 )",
    "(67 (0x23 ({msg1} ({h1} )",
    Ex::Fail
)]
#[case(
    "(66 (0x23 ({msg1} ({h1} (123 )",
    "(67 (0x23 ({msg2} ({h1} )",
    Ex::Fail
)]
//    wrong puzzle
#[case(
    "(66 (0x23 ({msg1} ({h2} (123 )",
    "(67 (0x23 ({msg1} ({h1} )",
    Ex::Fail
)]
//    wrong amount
#[case(
    "(66 (0x23 ({msg1} ({h1} (122 )",
    "(67 (0x23 ({msg1} ({h1} )",
    Ex::Fail
)]
//    wrong parent
#[case(
    "(66 (0x23 ({msg1} ({h1} (123 )",
    "(67 (0x23 ({msg1} ({h2} )",
    Ex::Fail
)]
// sender parent & puzzle and receiver puzzle committment
#[case(
    "(66 (0x32 ({msg1} ({h1} )",
    "(67 (0x32 ({msg1} ({h1} ({h2} )",
    Ex::Pass
)]
#[case(
    "(67 (0x32 ({msg1} ({h2} ({h1} )",
    "(66 (0x32 ({msg1} ({h2} )",
    Ex::Pass
)]
//    wrong messages
#[case(
    "(66 (0x32 ({msg2} ({h1} )",
    "(67 (0x32 ({msg1} ({h1} ({h2} )",
    Ex::Fail
)]
#[case(
    "(66 (0x32 ({msg1} ({h1} )",
    "(67 (0x32 ({msg2} ({h1} ({h2} )",
    Ex::Fail
)]
//    wrong puzzle
#[case(
    "(66 (0x32 ({msg1} ({h2} )",
    "(67 (0x32 ({msg1} ({h1} ({h2} )",
    Ex::Fail
)]
//    wrong puzzle
#[case(
    "(66 (0x32 ({msg1} ({h1} )",
    "(67 (0x32 ({msg1} ({h1} ({h1} )",
    Ex::Fail
)]
//    wrong parent
#[case(
    "(66 (0x32 ({msg1} ({h1} )",
    "(67 (0x32 ({msg1} ({h2} ({h2} )",
    Ex::Fail
)]
fn test_message_conditions_two_spends(
    #[case] coin1_case: &str,
    #[case] coin2_case: &str,
    #[case] expect: Ex,
) {
    let flags = MEMPOOL_MODE;
    let test = format!(
        "(\
        (({{h1}} ({{h2}} (123 (\
            ({coin1_case} \
            ))\
        (({{h2}} ({{h1}} (123 (\
            ({coin2_case} \
            ))\
        ))"
    );
    let ret = cond_test_flag(&test, flags);

    let expect_pass = match expect {
        Ex::Pass => true,
        Ex::Fail => false,
    };

    if let Ok((a, conds)) = ret {
        assert!(expect_pass);
        assert_eq!(conds.cost, 0);
        assert_eq!(conds.spends.len(), 2);

        let spend = &conds.spends[0];
        assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
        assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
        assert_eq!(spend.flags, 0);

        let spend = &conds.spends[1];
        assert_eq!(*spend.coin_id, test_coin_id(H2, H1, 123));
        assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H1);
        assert_eq!(spend.flags, 0);
    } else if expect_pass {
        panic!("failed: {:?}", ret.unwrap_err().1);
    } else {
        let actual_err = ret.unwrap_err().1;
        println!("Error: {actual_err:?}");
        assert_eq!(ErrorCode::MessageNotSentOrReceived, actual_err);
    }
}

// generates all positive test cases between two spends
#[test]
fn test_all_message_conditions() {
    for mode in 0..0b11_1111 {
        let coin1_case = match mode & 0b111 {
            0 => "",
            0b001 => "(456 ",
            0b010 => "({h1} ",
            0b011 => "({h1} (456 ",
            0b100 => "({h2} ",
            0b101 => "({h2} (456 ",
            0b110 => "({h2} ({h1} ",
            0b111 => "({coin21_456} ",
            _ => {
                panic!("unexpected {mode}");
            }
        };

        let coin2_case = match mode >> 3 {
            0 => "",
            0b001 => "(123 ",
            0b010 => "({h2} ",
            0b011 => "({h2} (123 ",
            0b100 => "({h1} ",
            0b101 => "({h1} (123 ",
            0b110 => "({h1} ({h2} ",
            0b111 => "({coin12} ",
            _ => {
                panic!("unexpected {mode}");
            }
        };

        let test = format!(
            "(\
        (({{h1}} ({{h2}} (123 (\
            ((66 ({mode} ({{msg2}} {coin1_case} )\
            ))\
        (({{h2}} ({{h1}} (456 (\
            ((67 ({mode} ({{msg2}} {coin2_case} )\
            ))\
        ))"
        );
        let (a, conds) = cond_test_flag(&test, 0).expect("condition expected to pass");

        assert_eq!(conds.cost, 0);
        assert_eq!(conds.spends.len(), 2);

        let spend = &conds.spends[0];
        assert_eq!(*spend.coin_id, test_coin_id(H1, H2, 123));
        assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H2);
        assert_eq!(spend.flags, 0);

        let spend = &conds.spends[1];
        assert_eq!(*spend.coin_id, test_coin_id(H2, H1, 456));
        assert_eq!(a.atom(spend.puzzle_hash).as_ref(), H1);
        assert_eq!(spend.flags, 0);
    }
}

#[test]
fn test_message_eligible_for_ff() {
    for mode in 0..0b11_1111 {
        let coin1_case = match mode & 0b111 {
            0 => "",
            0b001 => "(123 ",
            0b010 => "({h1} ",
            0b011 => "({h1} (123 ",
            0b100 => "({h2} ",
            0b101 => "({h2} (123 ",
            0b110 => "({h2} ({h1} ",
            0b111 => "({coin21} ",
            _ => {
                panic!("unexpected {mode}");
            }
        };

        let coin2_case = match mode >> 3 {
            0 => "",
            0b001 => "(123 ",
            0b010 => "({h2} ",
            0b011 => "({h2} (123 ",
            0b100 => "({h1} ",
            0b101 => "({h1} (123 ",
            0b110 => "({h1} ({h2} ",
            0b111 => "({coin12} ",
            _ => {
                panic!("unexpected {mode}");
            }
        };

        // this is a model example of a spend that's eligible for FF
        // it mimics the output of singleton_top_layer_v1_1
        // The first test is where the sender is the spend that may be
        // fast-forwarded
        // 73=ASSERT_MY_AMOUNT
        // 71=ASSERT_MY_PARENT_ID
        // 51=CREATE_COIN
        // 66=SEND_MESSAGE
        // 67=RECEIVE_MESSAGE
        let test = format!(
            "(\
       (({{h1}} ({{h2}} (123 (\
           ((73 (123 ) \
           ((71 ({{h1}} ) \
           ((51 ({{h2}} (123 ) \
           ((66 ({mode} ({{msg2}} {coin1_case} )\
           ))\
       (({{h2}} ({{h1}} (123 (\
           ((67 ({mode} ({{msg2}} {coin2_case} )\
           ))\
       ))"
        );

        let (_a, cond) = cond_test_flag(&test, 0).expect("cond_test");
        assert!(cond.spends.len() == 2);
        assert_eq!(
            (cond.spends[0].flags & ELIGIBLE_FOR_FF) != 0,
            (mode & 0b10_0000) == 0
        );
        assert_eq!((cond.spends[1].flags & ELIGIBLE_FOR_FF), 0);

        // flip sender and receiver. The receiving spend is the one eligible for
        // fast-forwarding
        let test = format!(
            "(\
       (({{h2}} ({{h1}} (123 (\
           ((73 (123 ) \
           ((71 ({{h2}} ) \
           ((51 ({{h1}} (123 ) \
           ((67 ({mode} ({{msg2}} {coin2_case} )\
           ))\
       (({{h1}} ({{h2}} (123 (\
           ((66 ({mode} ({{msg2}} {coin1_case} )\
           ))\
       ))"
        );

        let (_a, cond) = cond_test_flag(&test, 0).expect("cond_test");
        assert!(cond.spends.len() == 2);
        assert_eq!(
            (cond.spends[0].flags & ELIGIBLE_FOR_FF) != 0,
            (mode & 0b100) == 0
        );
        assert_eq!((cond.spends[1].flags & ELIGIBLE_FOR_FF), 0);
    }
}
