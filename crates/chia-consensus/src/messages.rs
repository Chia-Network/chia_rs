use crate::condition_sanitizers::sanitize_hash;
use crate::sanitize_int::{sanitize_uint, SanitizedUint};
use crate::validation_error::{first, rest, ErrorCode, ValidationErr};
use chia_protocol::Bytes32;
use clvmr::{Allocator, NodePtr};
use std::sync::Arc;

// these are mode flags used as the first argument to SEND_MESSAGE and
// RECEIVE_MESSAGE. They indicate which properties of the sender and receiver we
// commit to, that must match. The mode flags for the sender are shifted left 3 bits.
pub const PARENT: u8 = 0b100;
pub const PUZZLE: u8 = 0b010;
pub const AMOUNT: u8 = 0b001;
pub const PUZZLEAMOUNT: u8 = 0b011;
pub const PARENTAMOUNT: u8 = 0b101;
pub const PARENTPUZZLE: u8 = 0b110;
pub const COINID: u8 = 0b111;

#[derive(Debug)]
pub enum SpendId {
    OwnedCoinId(Arc<Bytes32>),
    CoinId(NodePtr),
    Parent(NodePtr),
    Puzzle(NodePtr),
    Amount(u64),
    PuzzleAmount(NodePtr, u64),
    ParentAmount(NodePtr, u64),
    ParentPuzzle(NodePtr, NodePtr),
    None,
}

impl SpendId {
    // args is an in-out parameter. It's updated to point to then next argument
    pub fn parse(a: &Allocator, args: &mut NodePtr, mode: u8) -> Result<SpendId, ValidationErr> {
        // we have a special case for when all three mode flags are set. That means
        // we're committing to parent, puzzle and amount. In this case you just
        // specify the coin ID
        if mode == COINID {
            let coinid = sanitize_hash(a, first(a, *args)?, 32, ErrorCode::InvalidCoinId)?;
            *args = rest(a, *args)?;
            return Ok(Self::CoinId(coinid));
        }

        let parent = if (mode & PARENT) != 0 {
            let parent = sanitize_hash(a, first(a, *args)?, 32, ErrorCode::InvalidParentId)?;
            *args = rest(a, *args)?;
            parent
        } else {
            NodePtr::NIL
        };

        let puzzle = if (mode & PUZZLE) != 0 {
            let puzzle = sanitize_hash(a, first(a, *args)?, 32, ErrorCode::InvalidPuzzleHash)?;
            *args = rest(a, *args)?;
            puzzle
        } else {
            NodePtr::NIL
        };

        let amount = if (mode & AMOUNT) != 0 {
            let amount = match sanitize_uint(a, first(a, *args)?, 8, ErrorCode::InvalidCoinAmount)?
            {
                SanitizedUint::PositiveOverflow => {
                    return Err(ValidationErr(*args, ErrorCode::CoinAmountExceedsMaximum));
                }
                SanitizedUint::NegativeOverflow => {
                    return Err(ValidationErr(*args, ErrorCode::CoinAmountNegative));
                }
                SanitizedUint::Ok(amount) => amount,
            };
            *args = rest(a, *args)?;
            amount
        } else {
            0
        };

        match mode {
            PARENT => Ok(Self::Parent(parent)),
            PUZZLE => Ok(Self::Puzzle(puzzle)),
            AMOUNT => Ok(Self::Amount(amount)),
            PARENTPUZZLE => Ok(Self::ParentPuzzle(parent, puzzle)),
            PARENTAMOUNT => Ok(Self::ParentAmount(parent, amount)),
            PUZZLEAMOUNT => Ok(Self::PuzzleAmount(puzzle, amount)),
            0 => Ok(Self::None),
            _ => Err(ValidationErr(*args, ErrorCode::InvalidMessageMode)),
        }
    }

    pub fn from_self(
        mode: u8,
        parent: NodePtr,
        puzzle: NodePtr,
        amount: u64,
        coin_id: &Arc<Bytes32>,
    ) -> Result<SpendId, ValidationErr> {
        if mode == COINID {
            return Ok(Self::OwnedCoinId(coin_id.clone()));
        }

        match mode {
            PARENT => Ok(Self::Parent(parent)),
            PUZZLE => Ok(Self::Puzzle(puzzle)),
            AMOUNT => Ok(Self::Amount(amount)),
            PARENTPUZZLE => Ok(Self::ParentPuzzle(parent, puzzle)),
            PARENTAMOUNT => Ok(Self::ParentAmount(parent, amount)),
            PUZZLEAMOUNT => Ok(Self::PuzzleAmount(puzzle, amount)),
            0 => Ok(Self::None),
            _ => Err(ValidationErr(NodePtr::NIL, ErrorCode::InvalidMessageMode)),
        }
    }

    pub fn make_key(&self, out: &mut Vec<u8>, a: &Allocator) {
        match self {
            Self::OwnedCoinId(coinid) => {
                out.push(COINID);
                out.extend_from_slice(coinid);
            }
            Self::CoinId(coinid) => {
                out.push(COINID);
                out.extend_from_slice(a.atom(*coinid).as_ref());
            }
            Self::Parent(parent) => {
                out.push(PARENT);
                out.extend_from_slice(a.atom(*parent).as_ref());
            }
            Self::Puzzle(puzzle) => {
                out.push(PUZZLE);
                out.extend_from_slice(a.atom(*puzzle).as_ref());
            }
            Self::Amount(amount) => {
                out.push(AMOUNT);
                out.extend_from_slice(&amount.to_be_bytes());
            }
            Self::PuzzleAmount(puzzle, amount) => {
                out.push(PUZZLEAMOUNT);
                out.extend_from_slice(a.atom(*puzzle).as_ref());
                out.extend_from_slice(&amount.to_be_bytes());
            }
            Self::ParentAmount(parent, amount) => {
                out.push(PARENTAMOUNT);
                out.extend_from_slice(a.atom(*parent).as_ref());
                out.extend_from_slice(&amount.to_be_bytes());
            }
            Self::ParentPuzzle(parent, puzzle) => {
                out.push(PARENTPUZZLE);
                out.extend_from_slice(a.atom(*parent).as_ref());
                out.extend_from_slice(a.atom(*puzzle).as_ref());
            }
            Self::None => {
                out.push(0);
            }
        }
    }
}

pub struct Message {
    pub src: SpendId,
    pub dst: SpendId,
    pub msg: NodePtr,
    pub counter: i8,
}

impl Message {
    pub fn make_key(&self, a: &Allocator) -> Vec<u8> {
        let mut key = Vec::<u8>::with_capacity((1 + 32 + 32) * 2 + 32);
        self.src.make_key(&mut key, a);
        self.dst.make_key(&mut key, a);
        key.extend_from_slice(a.atom(self.msg).as_ref());
        key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;
    use rstest::rstest;

    const BUF0: [u8; 32] = hex!("0000000000000000000000000000000000000000000000000000000000000000");
    const BUF1: [u8; 32] = hex!("0101010101010101010101010101010101010101010101010101010101010101");
    const BUF2: [u8; 32] = hex!("0202020202020202020202020202020202020202020202020202020202020202");

    #[rstest]
    #[case(0b000, "00")]
    #[case(0b001, "010000000000000539")]
    #[case(
        0b010,
        "020101010101010101010101010101010101010101010101010101010101010101"
    )]
    #[case(
        0b100,
        "040000000000000000000000000000000000000000000000000000000000000000"
    )]
    #[case(0b110, "0600000000000000000000000000000000000000000000000000000000000000000101010101010101010101010101010101010101010101010101010101010101")]
    #[case(
        0b111,
        "070202020202020202020202020202020202020202020202020202020202020202"
    )]
    #[case(
        0b011,
        "0301010101010101010101010101010101010101010101010101010101010101010000000000000539"
    )]
    #[case(
        0b101,
        "0500000000000000000000000000000000000000000000000000000000000000000000000000000539"
    )]
    fn test_from_self(#[case] mode: u8, #[case] expected: &str) {
        let mut a = Allocator::new();
        let parent = a.new_atom(&BUF0).unwrap();
        let puzzle = a.new_atom(&BUF1).unwrap();
        let coin_id = Arc::<Bytes32>::new(Bytes32::new(BUF2));
        let src = SpendId::from_self(mode, parent, puzzle, 1337, &coin_id).unwrap();

        let mut key = Vec::<u8>::new();
        src.make_key(&mut key, &a);
        assert_eq!(key, hex::decode(expected).unwrap());
    }

    #[rstest]
    #[case(0b000, "00")]
    #[case(0b001, "010000000000000539")]
    #[case(
        0b010,
        "020101010101010101010101010101010101010101010101010101010101010101"
    )]
    #[case(
        0b100,
        "040000000000000000000000000000000000000000000000000000000000000000"
    )]
    #[case(0b110, "0600000000000000000000000000000000000000000000000000000000000000000101010101010101010101010101010101010101010101010101010101010101")]
    #[case(
        0b111,
        "070202020202020202020202020202020202020202020202020202020202020202"
    )]
    #[case(
        0b011,
        "0301010101010101010101010101010101010101010101010101010101010101010000000000000539"
    )]
    #[case(
        0b101,
        "0500000000000000000000000000000000000000000000000000000000000000000000000000000539"
    )]
    fn test_parse(#[case] mode: u8, #[case] expected: &str) {
        let mut a = Allocator::new();
        let mut args = NodePtr::NIL;
        if mode == COINID {
            let value = a.new_atom(&BUF2).unwrap();
            args = a.new_pair(value, args).unwrap();
        } else {
            if (mode & AMOUNT) != 0 {
                let value = a.new_small_number(1337).unwrap();
                args = a.new_pair(value, args).unwrap();
            }
            if (mode & PUZZLE) != 0 {
                let value = a.new_atom(&BUF1).unwrap();
                args = a.new_pair(value, args).unwrap();
            }
            if (mode & PARENT) != 0 {
                let value = a.new_atom(&BUF0).unwrap();
                args = a.new_pair(value, args).unwrap();
            }
        }
        let src = SpendId::parse(&a, &mut args, mode).unwrap();
        // ensure we parsed all arguments
        assert!(a.atom_eq(args, NodePtr::NIL));

        let mut key = Vec::<u8>::new();
        src.make_key(&mut key, &a);
        assert_eq!(key, hex::decode(expected).unwrap());
    }
}
