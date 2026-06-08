//! Tri-state optional type for Streamable serialization.
//!
//! Wire encoding:
//! - `None(Tail)`      → prefix byte 0x00, then Tail streamed
//! - `Some1(V1, Tail)` → prefix byte 0x01, then V1 streamed, then Tail streamed
//! - `Some2(V2)`       → prefix byte 0x02, then V2 streamed (NO Tail)
//!
//! When Tail = (), the unit type serializes as zero bytes, so Option3<V1, V2>
//! is wire-identical to Option3<V1, V2, ()>.

use crate::Streamable;
use crate::chia_error::{Error, Result};
use chia_sha2::Sha256;
use std::io::Cursor;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum Option3<V1, V2, Tail = ()> {
    None(Tail),
    Some1(V1, Tail),
    Some2(V2),
}

impl<V1, V2, Tail: Default> Default for Option3<V1, V2, Tail> {
    fn default() -> Self {
        Option3::None(Tail::default())
    }
}

impl<V1: Streamable, V2: Streamable, Tail: Streamable> Streamable for Option3<V1, V2, Tail> {
    fn update_digest(&self, digest: &mut Sha256) {
        match self {
            Option3::None(tail) => {
                digest.update([0]);
                tail.update_digest(digest);
            }
            Option3::Some1(v, tail) => {
                digest.update([1]);
                v.update_digest(digest);
                tail.update_digest(digest);
            }
            Option3::Some2(v) => {
                digest.update([2]);
                v.update_digest(digest);
            }
        }
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        match self {
            Option3::None(tail) => {
                out.push(0);
                tail.stream(out)?;
            }
            Option3::Some1(v, tail) => {
                out.push(1);
                v.stream(out)?;
                tail.stream(out)?;
            }
            Option3::Some2(v) => {
                out.push(2);
                v.stream(out)?;
            }
        }
        Ok(())
    }

    fn parse<const TRUSTED: bool>(input: &mut Cursor<&[u8]>) -> Result<Self> {
        let val = crate::read_bytes(input, 1)?[0];
        match val {
            0 => Ok(Option3::None(Tail::parse::<TRUSTED>(input)?)),
            1 => Ok(Option3::Some1(V1::parse::<TRUSTED>(input)?, Tail::parse::<TRUSTED>(input)?)),
            2 => Ok(Option3::Some2(V2::parse::<TRUSTED>(input)?)),
            _ => Err(Error::InvalidOptional),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<
        V1: Streamable + PartialEq + std::fmt::Debug,
        V2: Streamable + PartialEq + std::fmt::Debug,
    >(
        opt: Option3<V1, V2>,
    ) {
        let bytes = opt.to_bytes().unwrap();
        let parsed = Option3::<V1, V2>::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, opt);
    }

    #[test]
    fn test_none() {
        let opt: Option3<u32, u64> = Option3::None(());
        assert_eq!(opt.to_bytes().unwrap(), vec![0x00]);
        round_trip(opt);
    }

    #[test]
    fn test_some1() {
        let opt: Option3<u32, u64> = Option3::Some1(0x1337_u32, ());
        assert_eq!(opt.to_bytes().unwrap(), vec![0x01, 0x00, 0x00, 0x13, 0x37]);
        round_trip(opt);
    }

    #[test]
    fn test_some2() {
        let opt = Option3::<u32, u64>::Some2(0xCAFEBABE_u64);
        let bytes = opt.to_bytes().unwrap();
        assert_eq!(bytes[0], 0x02);
        round_trip(opt);
    }

    #[test]
    fn test_invalid_prefix() {
        let bytes = vec![0x03];
        assert!(Option3::<u32, u64>::from_bytes(&bytes).is_err());
    }

    #[test]
    fn test_with_tail_none() {
        let opt: Option3<u32, u64, Vec<u8>> = Option3::None(vec![1, 2, 3]);
        let bytes = opt.to_bytes().unwrap();
        // 0x00 prefix + 4-byte length (3) + 3 bytes
        assert_eq!(bytes[0], 0x00);
        assert_eq!(bytes.len(), 1 + 4 + 3);
        let parsed = Option3::<u32, u64, Vec<u8>>::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, opt);
    }

    #[test]
    fn test_with_tail_some1() {
        let opt: Option3<u32, u64, Vec<u8>> = Option3::Some1(0x42, vec![10, 20]);
        let bytes = opt.to_bytes().unwrap();
        // 0x01 prefix + 4-byte u32 + 4-byte length (2) + 2 bytes
        assert_eq!(bytes[0], 0x01);
        assert_eq!(bytes.len(), 1 + 4 + 4 + 2);
        let parsed = Option3::<u32, u64, Vec<u8>>::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, opt);
    }

    #[test]
    fn test_with_tail_some2_no_tail() {
        let opt: Option3<u32, u64, Vec<u8>> = Option3::Some2(0xDEADBEEF);
        let bytes = opt.to_bytes().unwrap();
        // 0x02 prefix + 8-byte u64 (NO tail)
        assert_eq!(bytes[0], 0x02);
        assert_eq!(bytes.len(), 1 + 8);
        let parsed = Option3::<u32, u64, Vec<u8>>::from_bytes(&bytes).unwrap();
        assert_eq!(parsed, opt);
    }
}
