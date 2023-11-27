use num_bigint::BigInt;

use crate::{ClvmDecoder, ClvmEncoder, FromClvm, FromClvmError, ToClvm, ToClvmError};

#[derive(Debug, Copy, Clone)]
pub struct MatchByte<const BYTE: u8>;

impl<N, const BYTE: u8> ToClvm<N> for MatchByte<BYTE> {
    fn to_clvm(&self, encoder: &mut impl ClvmEncoder<Node = N>) -> Result<N, ToClvmError> {
        if BYTE == 0 {
            return encoder.encode_atom(&[]);
        }
        let number = BigInt::from(BYTE);
        let bytes = number.to_signed_bytes_be();
        encoder.encode_atom(&bytes)
    }
}

impl<N, const BYTE: u8> FromClvm<N> for MatchByte<BYTE> {
    fn from_clvm(decoder: &impl ClvmDecoder<Node = N>, node: N) -> Result<Self, FromClvmError> {
        match decoder.decode_atom(&node)? {
            [] if BYTE == 0 => Ok(Self),
            [byte] if *byte == BYTE && BYTE > 0 => Ok(Self),
            _ => Err(FromClvmError::Custom(format!(
                "expected an atom with a single byte value of {}",
                BYTE
            ))),
        }
    }
}
