use crate::gen::flags::ENABLE_ASSERT_BEFORE;
use clvmr::allocator::{Allocator, NodePtr, SExp};
use clvmr::cost::Cost;

pub type ConditionOpcode = u8;

// AGG_SIG is ascii "1"
pub const AGG_SIG_UNSAFE: ConditionOpcode = 49;
pub const AGG_SIG_ME: ConditionOpcode = 50;

// the conditions below reserve coin amounts and have to be accounted for in
// output totals
pub const CREATE_COIN: ConditionOpcode = 51;
pub const RESERVE_FEE: ConditionOpcode = 52;

// the conditions below deal with announcements, for inter-coin communication
pub const CREATE_COIN_ANNOUNCEMENT: ConditionOpcode = 60;
pub const ASSERT_COIN_ANNOUNCEMENT: ConditionOpcode = 61;
pub const CREATE_PUZZLE_ANNOUNCEMENT: ConditionOpcode = 62;
pub const ASSERT_PUZZLE_ANNOUNCEMENT: ConditionOpcode = 63;
pub const ASSERT_CONCURRENT_SPEND: ConditionOpcode = 64;
pub const ASSERT_CONCURRENT_PUZZLE: ConditionOpcode = 65;

// the conditions below let coins inquire about themselves
pub const ASSERT_MY_COIN_ID: ConditionOpcode = 70;
pub const ASSERT_MY_PARENT_ID: ConditionOpcode = 71;
pub const ASSERT_MY_PUZZLEHASH: ConditionOpcode = 72;
pub const ASSERT_MY_AMOUNT: ConditionOpcode = 73;
pub const ASSERT_MY_BIRTH_SECONDS: ConditionOpcode = 74;
pub const ASSERT_MY_BIRTH_HEIGHT: ConditionOpcode = 75;

// the conditions below ensure that we're "far enough" in the future
// wall-clock time
pub const ASSERT_SECONDS_RELATIVE: ConditionOpcode = 80;
pub const ASSERT_SECONDS_ABSOLUTE: ConditionOpcode = 81;

// block index
pub const ASSERT_HEIGHT_RELATIVE: ConditionOpcode = 82;
pub const ASSERT_HEIGHT_ABSOLUTE: ConditionOpcode = 83;

// spend is not valid if block timestamp exceeds the specified one
pub const ASSERT_BEFORE_SECONDS_RELATIVE: ConditionOpcode = 84;
pub const ASSERT_BEFORE_SECONDS_ABSOLUTE: ConditionOpcode = 85;

// spend is not valid if block height exceeds the specified height
pub const ASSERT_BEFORE_HEIGHT_RELATIVE: ConditionOpcode = 86;
pub const ASSERT_BEFORE_HEIGHT_ABSOLUTE: ConditionOpcode = 87;

// no-op condition
pub const ALWAYS_TRUE: ConditionOpcode = 1;

pub const CREATE_COIN_COST: Cost = 1800000;
pub const AGG_SIG_COST: Cost = 1200000;

pub fn parse_opcode(a: &Allocator, op: NodePtr, flags: u32) -> Option<ConditionOpcode> {
    let buf = match a.sexp(op) {
        SExp::Atom(_) => a.atom(op),
        _ => return None,
    };
    if buf.len() != 1 {
        return None;
    }

    match buf[0] {
        AGG_SIG_UNSAFE
        | AGG_SIG_ME
        | CREATE_COIN
        | RESERVE_FEE
        | CREATE_COIN_ANNOUNCEMENT
        | ASSERT_COIN_ANNOUNCEMENT
        | CREATE_PUZZLE_ANNOUNCEMENT
        | ASSERT_PUZZLE_ANNOUNCEMENT
        | ASSERT_MY_COIN_ID
        | ASSERT_MY_PARENT_ID
        | ASSERT_MY_PUZZLEHASH
        | ASSERT_MY_AMOUNT
        | ASSERT_SECONDS_RELATIVE
        | ASSERT_SECONDS_ABSOLUTE
        | ASSERT_HEIGHT_RELATIVE
        | ASSERT_HEIGHT_ABSOLUTE
        | ALWAYS_TRUE => Some(buf[0]),
        _ => {
            if (flags & ENABLE_ASSERT_BEFORE) != 0 {
                match buf[0] {
                    ASSERT_BEFORE_SECONDS_RELATIVE
                    | ASSERT_BEFORE_SECONDS_ABSOLUTE
                    | ASSERT_BEFORE_HEIGHT_RELATIVE
                    | ASSERT_BEFORE_HEIGHT_ABSOLUTE
                    | ASSERT_CONCURRENT_SPEND
                    | ASSERT_CONCURRENT_PUZZLE
                    | ASSERT_MY_BIRTH_SECONDS
                    | ASSERT_MY_BIRTH_HEIGHT => Some(buf[0]),
                    _ => None,
                }
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
fn opcode_tester(a: &mut Allocator, val: &[u8]) -> Option<ConditionOpcode> {
    let v = a.new_atom(val).unwrap();
    parse_opcode(&a, v, 0)
}

#[cfg(test)]
fn opcode_tester_with_assert_before(a: &mut Allocator, val: &[u8]) -> Option<ConditionOpcode> {
    let v = a.new_atom(val).unwrap();
    parse_opcode(&a, v, ENABLE_ASSERT_BEFORE)
}

#[cfg(test)]
use rstest::rstest;

#[cfg(test)]
#[rstest]
// leading zeros are not allowed, it makes it a different value
#[case(&[ASSERT_HEIGHT_ABSOLUTE, 0], None, None)]
#[case(&[0, ASSERT_HEIGHT_ABSOLUTE], None, None)]
#[case(&[0], None, None)]
// all condition codes
#[case(&[AGG_SIG_UNSAFE], Some(AGG_SIG_UNSAFE), Some(AGG_SIG_UNSAFE))]
#[case(&[AGG_SIG_ME], Some(AGG_SIG_ME), Some(AGG_SIG_ME))]
#[case(&[CREATE_COIN], Some(CREATE_COIN), Some(CREATE_COIN))]
#[case(&[RESERVE_FEE], Some(RESERVE_FEE), Some(RESERVE_FEE))]
#[case(&[CREATE_COIN_ANNOUNCEMENT], Some(CREATE_COIN_ANNOUNCEMENT), Some(CREATE_COIN_ANNOUNCEMENT))]
#[case(&[ASSERT_COIN_ANNOUNCEMENT], Some(ASSERT_COIN_ANNOUNCEMENT), Some(ASSERT_COIN_ANNOUNCEMENT))]
#[case(&[CREATE_PUZZLE_ANNOUNCEMENT], Some(CREATE_PUZZLE_ANNOUNCEMENT), Some(CREATE_PUZZLE_ANNOUNCEMENT))]
#[case(&[ASSERT_PUZZLE_ANNOUNCEMENT], Some(ASSERT_PUZZLE_ANNOUNCEMENT), Some(ASSERT_PUZZLE_ANNOUNCEMENT))]
#[case(&[ASSERT_CONCURRENT_SPEND], None, Some(ASSERT_CONCURRENT_SPEND))]
#[case(&[ASSERT_CONCURRENT_PUZZLE], None, Some(ASSERT_CONCURRENT_PUZZLE))]
#[case(&[ASSERT_MY_COIN_ID], Some(ASSERT_MY_COIN_ID), Some(ASSERT_MY_COIN_ID))]
#[case(&[ASSERT_MY_PARENT_ID], Some(ASSERT_MY_PARENT_ID), Some(ASSERT_MY_PARENT_ID))]
#[case(&[ASSERT_MY_PUZZLEHASH], Some(ASSERT_MY_PUZZLEHASH), Some(ASSERT_MY_PUZZLEHASH))]
#[case(&[ASSERT_MY_AMOUNT], Some(ASSERT_MY_AMOUNT), Some(ASSERT_MY_AMOUNT))]
#[case(&[ASSERT_MY_BIRTH_SECONDS], None, Some(ASSERT_MY_BIRTH_SECONDS))]
#[case(&[ASSERT_MY_BIRTH_HEIGHT], None, Some(ASSERT_MY_BIRTH_HEIGHT))]
#[case(&[ASSERT_SECONDS_RELATIVE],Some(ASSERT_SECONDS_RELATIVE) , Some(ASSERT_SECONDS_RELATIVE))]
#[case(&[ASSERT_SECONDS_ABSOLUTE],Some(ASSERT_SECONDS_ABSOLUTE) , Some(ASSERT_SECONDS_ABSOLUTE))]
#[case(&[ASSERT_HEIGHT_RELATIVE], Some(ASSERT_HEIGHT_RELATIVE), Some(ASSERT_HEIGHT_RELATIVE))]
#[case(&[ASSERT_HEIGHT_ABSOLUTE], Some(ASSERT_HEIGHT_ABSOLUTE), Some(ASSERT_HEIGHT_ABSOLUTE))]
#[case(&[ASSERT_BEFORE_SECONDS_RELATIVE], None, Some(ASSERT_BEFORE_SECONDS_RELATIVE))]
#[case(&[ASSERT_BEFORE_SECONDS_ABSOLUTE], None, Some(ASSERT_BEFORE_SECONDS_ABSOLUTE))]
#[case(&[ASSERT_BEFORE_HEIGHT_RELATIVE], None, Some(ASSERT_BEFORE_HEIGHT_RELATIVE))]
#[case(&[ASSERT_BEFORE_HEIGHT_ABSOLUTE], None, Some(ASSERT_BEFORE_HEIGHT_ABSOLUTE))]
#[case(&[ALWAYS_TRUE], Some(ALWAYS_TRUE), Some(ALWAYS_TRUE))]

fn test_parse_opcode(
    #[case] input: &[u8],
    #[case] expected: Option<ConditionOpcode>,
    #[case] expected2: Option<ConditionOpcode>,
) {
    let mut a = Allocator::new();
    assert_eq!(opcode_tester(&mut a, input), expected);
    assert_eq!(opcode_tester_with_assert_before(&mut a, input), expected2);
}

#[test]
fn test_parse_invalid_opcode() {
    // a pair is never a valid condition
    let mut a = Allocator::new();
    let v1 = a.new_atom(&[0]).unwrap();
    let v2 = a.new_atom(&[0]).unwrap();
    let p = a.new_pair(v1, v2).unwrap();
    assert_eq!(parse_opcode(&a, p, 0), None);
    assert_eq!(parse_opcode(&a, p, ENABLE_ASSERT_BEFORE), None);
}
