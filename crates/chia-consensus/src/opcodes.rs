use clvmr::allocator::{Allocator, NodePtr, SExp};
use clvmr::cost::Cost;

pub type ConditionOpcode = u16;

// AGG_SIG is ascii "1"
pub const AGG_SIG_PARENT: ConditionOpcode = 43;
pub const AGG_SIG_PUZZLE: ConditionOpcode = 44;
pub const AGG_SIG_AMOUNT: ConditionOpcode = 45;
pub const AGG_SIG_PUZZLE_AMOUNT: ConditionOpcode = 46;
pub const AGG_SIG_PARENT_AMOUNT: ConditionOpcode = 47;
pub const AGG_SIG_PARENT_PUZZLE: ConditionOpcode = 48;
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

pub const SEND_MESSAGE: ConditionOpcode = 66;
pub const RECEIVE_MESSAGE: ConditionOpcode = 67;

// the conditions below let coins inquire about themselves
pub const ASSERT_MY_COIN_ID: ConditionOpcode = 70;
pub const ASSERT_MY_PARENT_ID: ConditionOpcode = 71;
pub const ASSERT_MY_PUZZLEHASH: ConditionOpcode = 72;
pub const ASSERT_MY_AMOUNT: ConditionOpcode = 73;
pub const ASSERT_MY_BIRTH_SECONDS: ConditionOpcode = 74;
pub const ASSERT_MY_BIRTH_HEIGHT: ConditionOpcode = 75;
pub const ASSERT_EPHEMERAL: ConditionOpcode = 76;

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
pub const REMARK: ConditionOpcode = 1;

// takes its cost as the first parameter, followed by future extensions
// the cost is specified in increments of 10000, to keep the values smaller
pub const SOFTFORK: ConditionOpcode = 90;

pub const CREATE_COIN_COST: Cost = 1_800_000;
pub const AGG_SIG_COST: Cost = 1_200_000;

pub const GENERIC_CONDITION_COST: Cost = 500;
pub const FREE_CONDITIONS: usize = 100;

// 2-byte condition opcodes have costs according to this table:

// the values `100 * (17 ** idx)/(16 ** idx)` rounded to three significant decimal figures

const fn calculate_cost_table() -> [u64; 256] {
    let (a, b) = (17, 16);
    let mut s = [0; 256];
    let (mut num, mut den) = (100_u64, 1_u64);
    let max = 1 << 59;
    let mut idx = 0;
    while idx < 256 {
        let v = num / den;
        let mut power_of_ten = 1000;
        while power_of_ten < v {
            power_of_ten *= 10;
        }
        power_of_ten /= 1000;
        s[idx] = (v / power_of_ten) * power_of_ten;
        num *= a;
        den *= b;
        while num > max {
            num >>= 5;
            den >>= 5;
        }
        idx += 1;
    }
    s
}

const COSTS: [Cost; 256] = calculate_cost_table();

pub fn compute_unknown_condition_cost(op: ConditionOpcode) -> Cost {
    if op < 256 {
        0
    } else {
        COSTS[(op & 0xff) as usize]
    }
}

pub fn parse_opcode(a: &Allocator, op: NodePtr, _flags: u32) -> Option<ConditionOpcode> {
    let buf = match a.sexp(op) {
        SExp::Atom => a.atom(op),
        SExp::Pair(..) => return None,
    };
    let buf = buf.as_ref();
    if buf.len() == 2 {
        if buf[0] == 0 {
            // no redundant leading zeroes
            None
        } else {
            // These are 2-byte condition codes whose first byte is non-zero
            Some(ConditionOpcode::from_be_bytes(buf.try_into().unwrap()))
        }
    } else if buf.len() == 1 {
        let b0 = ConditionOpcode::from(buf[0]);
        match b0 {
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
            | REMARK
            | ASSERT_BEFORE_SECONDS_RELATIVE
            | ASSERT_BEFORE_SECONDS_ABSOLUTE
            | ASSERT_BEFORE_HEIGHT_RELATIVE
            | ASSERT_BEFORE_HEIGHT_ABSOLUTE
            | ASSERT_CONCURRENT_SPEND
            | ASSERT_CONCURRENT_PUZZLE
            | ASSERT_MY_BIRTH_SECONDS
            | ASSERT_MY_BIRTH_HEIGHT
            | ASSERT_EPHEMERAL
            | SOFTFORK
            | AGG_SIG_PARENT
            | AGG_SIG_PUZZLE
            | AGG_SIG_AMOUNT
            | AGG_SIG_PUZZLE_AMOUNT
            | AGG_SIG_PARENT_AMOUNT
            | AGG_SIG_PARENT_PUZZLE
            | SEND_MESSAGE
            | RECEIVE_MESSAGE => Some(b0),
            _ => None,
        }
    } else {
        None
    }
}

#[cfg(test)]
fn opcode_tester(a: &mut Allocator, val: &[u8], flags: u32) -> Option<ConditionOpcode> {
    let v = a.new_atom(val).unwrap();
    parse_opcode(a, v, flags)
}

#[cfg(test)]
use rstest::rstest;

#[cfg(test)]
#[rstest]
// leading zeros are not allowed, it makes it a different value
#[case(&[ASSERT_HEIGHT_ABSOLUTE as u8, 0, 0], None)]
#[case(&[0, ASSERT_HEIGHT_ABSOLUTE as u8], None)]
#[case(&[0], None)]
// all condition codes
#[case(&[AGG_SIG_UNSAFE as u8], Some(AGG_SIG_UNSAFE))]
#[case(&[AGG_SIG_ME as u8], Some(AGG_SIG_ME))]
#[case(&[CREATE_COIN as u8], Some(CREATE_COIN))]
#[case(&[RESERVE_FEE as u8], Some(RESERVE_FEE))]
#[case(&[CREATE_COIN_ANNOUNCEMENT as u8], Some(CREATE_COIN_ANNOUNCEMENT))]
#[case(&[ASSERT_COIN_ANNOUNCEMENT as u8], Some(ASSERT_COIN_ANNOUNCEMENT))]
#[case(&[CREATE_PUZZLE_ANNOUNCEMENT as u8], Some(CREATE_PUZZLE_ANNOUNCEMENT))]
#[case(&[ASSERT_PUZZLE_ANNOUNCEMENT as u8], Some(ASSERT_PUZZLE_ANNOUNCEMENT))]
#[case(&[ASSERT_CONCURRENT_SPEND as u8], Some(ASSERT_CONCURRENT_SPEND))]
#[case(&[ASSERT_CONCURRENT_PUZZLE as u8], Some(ASSERT_CONCURRENT_PUZZLE))]
#[case(&[ASSERT_MY_COIN_ID as u8], Some(ASSERT_MY_COIN_ID))]
#[case(&[ASSERT_MY_PARENT_ID as u8], Some(ASSERT_MY_PARENT_ID))]
#[case(&[ASSERT_MY_PUZZLEHASH as u8], Some(ASSERT_MY_PUZZLEHASH))]
#[case(&[ASSERT_MY_AMOUNT as u8], Some(ASSERT_MY_AMOUNT))]
#[case(&[ASSERT_MY_BIRTH_SECONDS as u8], Some(ASSERT_MY_BIRTH_SECONDS))]
#[case(&[ASSERT_MY_BIRTH_HEIGHT as u8], Some(ASSERT_MY_BIRTH_HEIGHT))]
#[case(&[ASSERT_EPHEMERAL as u8], Some(ASSERT_EPHEMERAL))]
#[case(&[ASSERT_SECONDS_RELATIVE as u8], Some(ASSERT_SECONDS_RELATIVE))]
#[case(&[ASSERT_SECONDS_ABSOLUTE as u8], Some(ASSERT_SECONDS_ABSOLUTE))]
#[case(&[ASSERT_HEIGHT_RELATIVE as u8], Some(ASSERT_HEIGHT_RELATIVE))]
#[case(&[ASSERT_HEIGHT_ABSOLUTE as u8], Some(ASSERT_HEIGHT_ABSOLUTE))]
#[case(&[ASSERT_BEFORE_SECONDS_RELATIVE as u8], Some(ASSERT_BEFORE_SECONDS_RELATIVE))]
#[case(&[ASSERT_BEFORE_SECONDS_ABSOLUTE as u8], Some(ASSERT_BEFORE_SECONDS_ABSOLUTE))]
#[case(&[ASSERT_BEFORE_HEIGHT_RELATIVE as u8], Some(ASSERT_BEFORE_HEIGHT_RELATIVE))]
#[case(&[ASSERT_BEFORE_HEIGHT_ABSOLUTE as u8], Some(ASSERT_BEFORE_HEIGHT_ABSOLUTE))]
#[case(&[REMARK as u8], Some(REMARK))]
fn test_parse_opcode(#[case] input: &[u8], #[case] expected: Option<ConditionOpcode>) {
    let mut a = Allocator::new();
    assert_eq!(opcode_tester(&mut a, input, 0), expected);
    assert_eq!(opcode_tester(&mut a, input, 0), expected);
}

#[cfg(test)]
#[rstest]
#[case(&[AGG_SIG_UNSAFE as u8], Some(AGG_SIG_UNSAFE))]
#[case(&[AGG_SIG_ME as u8], Some(AGG_SIG_ME))]
#[case(&[CREATE_COIN as u8], Some(CREATE_COIN))]
// the SOFTOFORK and new AGG_SIG_* condition is only recognized when the flag is set
#[case(&[SOFTFORK as u8], Some(SOFTFORK))]
#[case(&[AGG_SIG_PARENT as u8], Some(AGG_SIG_PARENT))]
#[case(&[AGG_SIG_PUZZLE as u8], Some(AGG_SIG_PUZZLE))]
#[case(&[AGG_SIG_AMOUNT as u8], Some(AGG_SIG_AMOUNT))]
#[case(&[AGG_SIG_PUZZLE_AMOUNT as u8], Some(AGG_SIG_PUZZLE_AMOUNT))]
#[case(&[AGG_SIG_PARENT_AMOUNT as u8], Some(AGG_SIG_PARENT_AMOUNT))]
#[case(&[AGG_SIG_PARENT_PUZZLE as u8], Some(AGG_SIG_PARENT_PUZZLE))]
#[case(&[ASSERT_EPHEMERAL as u8], Some(ASSERT_EPHEMERAL))]
#[case(&[ASSERT_BEFORE_SECONDS_RELATIVE as u8], Some(ASSERT_BEFORE_SECONDS_RELATIVE))]
#[case(&[SEND_MESSAGE as u8], Some(SEND_MESSAGE))]
#[case(&[RECEIVE_MESSAGE as u8], Some(RECEIVE_MESSAGE))]
fn test_parse_opcode_softfork(#[case] input: &[u8], #[case] expected: Option<ConditionOpcode>) {
    let mut a = Allocator::new();
    assert_eq!(opcode_tester(&mut a, input, 0), expected);
}

#[test]
fn test_parse_invalid_opcode() {
    // a pair is never a valid condition
    let mut a = Allocator::new();
    let v1 = a.new_atom(&[0]).unwrap();
    let v2 = a.new_atom(&[0]).unwrap();
    let p = a.new_pair(v1, v2).unwrap();
    assert_eq!(parse_opcode(&a, p, 0), None);
}
