use num_bigint::BigInt;

use crate::{ClvmDecoder, ClvmEncoder, FromClvm, FromClvmError, ToClvm, ToClvmError};

#[derive(Debug, Copy, Clone)]
pub struct MatchByte<const BYTE: u8>;

impl<N, E: ClvmEncoder<Node = N>, const BYTE: u8> ToClvm<E> for MatchByte<BYTE> {
    fn to_clvm(&self, encoder: &mut E) -> Result<N, ToClvmError> {
        encoder.encode_bigint(BigInt::from(BYTE))
    }
}

impl<N, D: ClvmDecoder<Node = N>, const BYTE: u8> FromClvm<D> for MatchByte<BYTE> {
    fn from_clvm(decoder: &D, node: N) -> Result<Self, FromClvmError> {
        match decoder.decode_atom(&node)?.as_ref() {
            [] if BYTE == 0 => Ok(Self),
            [byte] if *byte == BYTE && BYTE > 0 => Ok(Self),
            _ => Err(FromClvmError::Custom(format!(
                "expected an atom with a single byte value of {BYTE}"
            ))),
        }
    }
}
