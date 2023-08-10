use clvmr::{
    allocator::{NodePtr, SExp},
    Allocator,
};
use num_bigint::{BigInt, Sign};

use crate::{Error, FromClvm, Result, ToClvm};

#[derive(Debug, Copy, Clone)]
pub struct MatchByte<const BYTE: u8>;

impl<const BYTE: u8> ToClvm for MatchByte<BYTE> {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        a.new_number(BYTE.into()).map_err(Error::Allocator)
    }
}

impl<const BYTE: u8> FromClvm for MatchByte<BYTE> {
    fn from_clvm(a: &Allocator, node: NodePtr) -> Result<Self> {
        if let SExp::Atom() = a.sexp(node) {
            let value: u8 = to_unsigned(a.number(node)).try_into().map_err(
                |error: <u8 as TryFrom<BigInt>>::Error| Error::Custom(error.to_string()),
            )?;

            if value == BYTE {
                Ok(Self)
            } else {
                Err(Error::Custom(format!(
                    "expected an atom with a value of {}",
                    BYTE
                )))
            }
        } else {
            Err(Error::ExpectedAtom(node))
        }
    }
}

fn to_unsigned(value: BigInt) -> BigInt {
    if value.sign() == Sign::Minus {
        value + 256
    } else {
        value
    }
}
