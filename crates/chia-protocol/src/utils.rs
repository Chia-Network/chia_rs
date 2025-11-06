use chia_sha2::Sha256;
use chia_traits::{Error, Result, Streamable};
use std::io::Cursor;

pub fn update_digest<T: Streamable, U: Streamable>(
    first: Option<&T>,
    second: Option<&U>,
    digest: &mut Sha256,
) {
    match (first, second) {
        (None, None) => {
            Streamable::update_digest(&0_u8, digest);
        }
        (Some(first), None) => {
            Streamable::update_digest(&1_u8, digest);
            first.update_digest(digest);
        }
        (None, Some(second)) => {
            Streamable::update_digest(&2_u8, digest);
            second.update_digest(digest);
        }
        (Some(first), Some(second)) => {
            Streamable::update_digest(&3_u8, digest);
            first.update_digest(digest);
            second.update_digest(digest);
        }
    }
}

// we serialize these two optionals in a backwards compatible way, by
// sharing the byte indicating whether the optional is set or not. A
// normal Optional uses 8 bits to store 1 bit. We use two bits to store
// two optionals. As long as the second one isn't set, the data
// strsucture is backward compatible with the previous RewardChainBlock
pub fn stream<T: Streamable, U: Streamable>(
    first: Option<&T>,
    second: Option<&U>,
    out: &mut Vec<u8>,
) -> Result<()> {
    match (first, second) {
        (None, None) => {
            Streamable::stream(&0_u8, out)?;
        }
        (Some(first), None) => {
            Streamable::stream(&1_u8, out)?;
            first.stream(out)?;
        }
        (None, Some(second)) => {
            Streamable::stream(&2_u8, out)?;
            second.stream(out)?;
        }
        (Some(first), Some(second)) => {
            Streamable::stream(&3_u8, out)?;
            first.stream(out)?;
            second.stream(out)?;
        }
    }
    Ok(())
}

pub fn parse<const TRUSTED: bool, T: Streamable, U: Streamable>(
    input: &mut Cursor<&[u8]>,
) -> Result<(Option<T>, Option<U>)> {
    let index = <u8 as Streamable>::parse::<TRUSTED>(input)?;
    Ok(match index {
        0 => (None, None),
        1 => (Some(T::parse::<TRUSTED>(input)?), None),
        2 => (None, Some(U::parse::<TRUSTED>(input)?)),
        3 => (
            Some(T::parse::<TRUSTED>(input)?),
            Some(U::parse::<TRUSTED>(input)?),
        ),
        _ => {
            return Err(Error::InvalidOptional);
        }
    })
}
