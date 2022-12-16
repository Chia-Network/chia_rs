use crate::chia_error::{Error, Result};
use sha2::{Digest, Sha256};
use std::convert::TryInto;
use std::io::Cursor;
use std::mem::size_of;

pub fn read_bytes<'a>(input: &'a mut Cursor<&[u8]>, len: usize) -> Result<&'a [u8]> {
    let pos = input.position();
    let buf: &'a [u8] = &input.get_ref()[pos as usize..];
    if buf.len() < len {
        Err(Error::EndOfBuffer)
    } else {
        let ret = &buf[..len];
        input.set_position(pos + len as u64);
        Ok(ret)
    }
}

#[test]
fn test_read_bytes() {
    let mut input = Cursor::<&[u8]>::new(&[0_u8, 1, 2, 3, 4]);
    assert_eq!(read_bytes(&mut input, 1).unwrap(), [0_u8]);
    assert_eq!(read_bytes(&mut input, 1).unwrap(), [1_u8]);
    assert_eq!(read_bytes(&mut input, 1).unwrap(), [2_u8]);
    assert_eq!(read_bytes(&mut input, 1).unwrap(), [3_u8]);
    assert_eq!(read_bytes(&mut input, 1).unwrap(), [4_u8]);
    assert_eq!(read_bytes(&mut input, 1).unwrap_err(), Error::EndOfBuffer);
}

pub trait Streamable {
    fn update_digest(&self, digest: &mut Sha256);
    fn stream(&self, out: &mut Vec<u8>) -> Result<()>;
    fn parse(input: &mut Cursor<&[u8]>) -> Result<Self>
    where
        Self: Sized;
}

macro_rules! streamable_primitive {
    ($t:ty) => {
        impl Streamable for $t {
            fn update_digest(&self, digest: &mut Sha256) {
                digest.update(&self.to_be_bytes());
            }
            fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
                Ok(out.extend_from_slice(&self.to_be_bytes()))
            }
            fn parse(input: &mut Cursor<&[u8]>) -> Result<Self> {
                let sz = size_of::<$t>();
                Ok(<$t>::from_be_bytes(
                    read_bytes(input, sz)?.try_into().unwrap(),
                ))
            }
        }
    };
}

streamable_primitive!(u8);
streamable_primitive!(i8);
streamable_primitive!(u16);
streamable_primitive!(i16);
streamable_primitive!(u32);
streamable_primitive!(i32);
streamable_primitive!(u64);
streamable_primitive!(i64);
streamable_primitive!(u128);
streamable_primitive!(i128);

impl<T: Streamable> Streamable for Vec<T> {
    fn update_digest(&self, digest: &mut Sha256) {
        (self.len() as u32).update_digest(digest);
        for e in self {
            e.update_digest(digest);
        }
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        if self.len() > u32::MAX as usize {
            Err(Error::InputTooLarge)
        } else {
            (self.len() as u32).stream(out)?;
            for e in self {
                e.stream(out)?;
            }
            Ok(())
        }
    }

    fn parse(input: &mut Cursor<&[u8]>) -> Result<Self> {
        let len = u32::parse(input)?;

        // TODO: pre-allocate capacity, but we'd need safe-guards for overflow
        // attacks
        let mut ret = Vec::<T>::new();
        for _ in 0..len {
            ret.push(T::parse(input)?);
        }
        Ok(ret)
    }
}

impl Streamable for String {
    fn update_digest(&self, digest: &mut Sha256) {
        let bytes = self.as_bytes();
        (bytes.len() as u32).update_digest(digest);
        digest.update(bytes);
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        // bytes is the UTF-8 sequence
        let bytes = self.bytes();
        if bytes.len() > u32::MAX as usize {
            Err(Error::InputTooLarge)
        } else {
            (bytes.len() as u32).stream(out)?;
            out.extend(bytes);
            Ok(())
        }
    }

    fn parse(input: &mut Cursor<&[u8]>) -> Result<Self> {
        let len = u32::parse(input)?;
        Ok(String::from(
            std::str::from_utf8(read_bytes(input, len as usize)?)
                .map_err(|_| Error::InvalidString)?,
        ))
    }
}

impl Streamable for bool {
    fn update_digest(&self, digest: &mut Sha256) {
        digest.update(if *self { [1] } else { [0] });
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        out.extend_from_slice(if *self { &[1] } else { &[0] });
        Ok(())
    }
    fn parse(input: &mut Cursor<&[u8]>) -> Result<Self> {
        let val = read_bytes(input, 1)?[0];
        match val {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::InvalidBool),
        }
    }
}

impl<T: Streamable> Streamable for Option<T> {
    fn update_digest(&self, digest: &mut Sha256) {
        match self {
            None => {
                digest.update([0]);
            }
            Some(v) => {
                digest.update([1]);
                v.update_digest(digest);
            }
        }
    }

    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        match self {
            None => {
                out.push(0);
            }
            Some(v) => {
                out.push(1);
                v.stream(out)?;
            }
        }
        Ok(())
    }
    fn parse(input: &mut Cursor<&[u8]>) -> Result<Self> {
        let val = read_bytes(input, 1)?[0];
        match val {
            0 => Ok(None),
            1 => Ok(Some(T::parse(input)?)),
            _ => Err(Error::InvalidOptional),
        }
    }
}

impl<T: Streamable, U: Streamable> Streamable for (T, U) {
    fn update_digest(&self, digest: &mut Sha256) {
        self.0.update_digest(digest);
        self.1.update_digest(digest);
    }
    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        self.0.stream(out)?;
        self.1.stream(out)?;
        Ok(())
    }
    fn parse(input: &mut Cursor<&[u8]>) -> Result<Self> {
        Ok((T::parse(input)?, U::parse(input)?))
    }
}

impl<T: Streamable, U: Streamable, V: Streamable> Streamable for (T, U, V) {
    fn update_digest(&self, digest: &mut Sha256) {
        self.0.update_digest(digest);
        self.1.update_digest(digest);
        self.2.update_digest(digest);
    }
    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        self.0.stream(out)?;
        self.1.stream(out)?;
        self.2.stream(out)?;
        Ok(())
    }
    fn parse(input: &mut Cursor<&[u8]>) -> Result<Self> {
        Ok((T::parse(input)?, U::parse(input)?, V::parse(input)?))
    }
}

impl<T: Streamable, U: Streamable, V: Streamable, W: Streamable> Streamable for (T, U, V, W) {
    fn update_digest(&self, digest: &mut Sha256) {
        self.0.update_digest(digest);
        self.1.update_digest(digest);
        self.2.update_digest(digest);
        self.3.update_digest(digest);
    }
    fn stream(&self, out: &mut Vec<u8>) -> Result<()> {
        self.0.stream(out)?;
        self.1.stream(out)?;
        self.2.stream(out)?;
        self.3.stream(out)?;
        Ok(())
    }
    fn parse(input: &mut Cursor<&[u8]>) -> Result<Self> {
        Ok((
            T::parse(input)?,
            U::parse(input)?,
            V::parse(input)?,
            W::parse(input)?,
        ))
    }
}

// ===== TESTS ====

#[cfg(test)]
use crate::bytes::{Bytes, Bytes32, Bytes48};

#[cfg(test)]
fn from_bytes<'de, T: Streamable + std::fmt::Debug + std::cmp::PartialEq>(
    buf: &'de [u8],
    expected: T,
) {
    let mut input = Cursor::<&[u8]>::new(buf);
    assert_eq!(T::parse(&mut input).unwrap(), expected);
}

#[cfg(test)]
fn from_bytes_fail<'de, T: Streamable + std::fmt::Debug + std::cmp::PartialEq>(
    buf: &'de [u8],
    expected: Error,
) {
    let mut input = Cursor::<&[u8]>::new(buf);
    assert_eq!(T::parse(&mut input).unwrap_err(), expected);
}

#[test]
fn test_parse_u64() {
    from_bytes::<u64>(&[0, 0, 0, 0, 0, 0, 0, 0], 0);
    from_bytes::<u64>(&[0, 0, 0, 0, 0, 0, 0, 1], 1);
    from_bytes::<u64>(&[0x80, 0, 0, 0, 0, 0, 0, 0], 0x8000000000000000);
    from_bytes::<u64>(
        &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        0xffffffffffffffff,
    );

    // truncated
    from_bytes_fail::<u64>(&[0, 0, 0, 0, 0, 0, 0], Error::EndOfBuffer);
    from_bytes_fail::<u64>(&[0, 0, 0, 0, 0, 0], Error::EndOfBuffer);
    from_bytes_fail::<u64>(&[0, 0, 0, 0, 0], Error::EndOfBuffer);
    from_bytes_fail::<u64>(&[0, 0, 0, 0], Error::EndOfBuffer);
    from_bytes_fail::<u64>(&[0, 0, 0], Error::EndOfBuffer);
    from_bytes_fail::<u64>(&[0, 0], Error::EndOfBuffer);
}

#[test]
fn test_parse_u128() {
    from_bytes::<u128>(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 0);
    from_bytes::<u128>(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1], 1);
    from_bytes::<u128>(
        &[0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        0x80000000000000000000000000000000,
    );
    from_bytes::<u128>(
        &[
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff,
        ],
        0xffffffffffffffffffffffffffffffff,
    );

    // truncated
    from_bytes_fail::<u128>(
        &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        Error::EndOfBuffer,
    );
    from_bytes_fail::<u128>(
        &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        Error::EndOfBuffer,
    );
    from_bytes_fail::<u128>(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], Error::EndOfBuffer);
    from_bytes_fail::<u128>(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], Error::EndOfBuffer);
    from_bytes_fail::<u128>(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], Error::EndOfBuffer);
    from_bytes_fail::<u128>(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0], Error::EndOfBuffer);
}

#[test]
fn test_parse_bytes32() {
    let buf: &[u8] = &[
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32,
    ];
    from_bytes::<Bytes32>(buf, Bytes32::from(buf));
    from_bytes_fail::<Bytes32>(&buf[0..30], Error::EndOfBuffer);
}

#[test]
fn test_parse_bytes48() {
    let buf: &[u8] = &[
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
    ];
    from_bytes::<Bytes48>(buf, Bytes48::from(buf));
    from_bytes_fail::<Bytes48>(&buf[0..47], Error::EndOfBuffer);
}

#[test]
fn test_parse_bytes_empty() {
    let buf: &[u8] = &[0, 0, 0, 0];
    from_bytes::<Bytes>(buf, [].to_vec().into());
}

#[test]
fn test_parse_bytes() {
    let buf: &[u8] = &[0, 0, 0, 3, 1, 2, 3];
    from_bytes::<Bytes>(buf, [1_u8, 2, 3].to_vec().into());
}

#[test]
fn test_parse_truncated_len() {
    let buf: &[u8] = &[0, 0, 1];
    from_bytes_fail::<Bytes>(buf, Error::EndOfBuffer);
}

#[test]
fn test_parse_truncated() {
    let buf: &[u8] = &[0, 0, 0, 4, 1, 2, 3];
    from_bytes_fail::<Bytes>(buf, Error::EndOfBuffer);
}

#[test]
fn test_parse_empty_list() {
    let buf: &[u8] = &[0, 0, 0, 0];
    from_bytes::<Vec<u32>>(buf, vec![]);
}

#[test]
fn test_parse_list_1() {
    let buf: &[u8] = &[0, 0, 0, 1, 1, 2, 3, 4];
    from_bytes::<Vec<u32>>(buf, vec![0x01020304]);
}

#[test]
fn test_parse_list_3() {
    let buf: &[u8] = &[0, 0, 0, 3, 1, 2, 3, 4, 1, 3, 3, 7, 0, 0, 4, 2];
    from_bytes::<Vec<u32>>(buf, vec![0x01020304, 0x01030307, 0x402]);
}

#[test]
fn test_parse_list_list_3() {
    let buf: &[u8] = &[
        0, 0, 0, 3, 0, 0, 0, 1, 1, 2, 3, 4, 0, 0, 0, 1, 1, 3, 3, 7, 0, 0, 0, 1, 0, 0, 4, 2,
    ];
    from_bytes::<Vec<Vec<u32>>>(buf, vec![vec![0x01020304], vec![0x01030307], vec![0x402]]);
}

#[test]
fn test_parse_long_list() {
    let buf: &[u8] = &[0xff, 0xff, 0xff, 0xff, 0, 0, 0];
    from_bytes_fail::<Vec<u32>>(buf, Error::EndOfBuffer);
}

#[test]
fn test_parse_tuple() {
    let buf: &[u8] = &[0, 0, 0, 3, 42, 0xff];
    from_bytes::<(u32, u8, i8)>(buf, (3, 42, -1));
}

#[test]
fn test_parse_nested_tuple() {
    let buf: &[u8] = &[0, 0, 0, 3, 42, 43, 44, 0xff, 0xff, 0xff, 0xff];
    from_bytes::<(u32, (u8, u8, u8), i32)>(buf, (3, (42, 43, 44), -1));
}

#[test]
fn test_parse_optional_clear() {
    let buf: &[u8] = &[0];
    from_bytes::<Option<u32>>(buf, None);
}

#[test]
fn test_parse_optional_zero() {
    let buf: &[u8] = &[1, 0];
    from_bytes::<Option<u8>>(buf, Some(0));
}

#[test]
fn test_parse_optional_u32() {
    let buf: &[u8] = &[1, 0, 0, 0x13, 0x37];
    from_bytes::<Option<u32>>(buf, Some(0x1337));
}

#[test]
fn test_parse_optional_str() {
    let buf: &[u8] = &[1, 0, 0, 0, 3, b'f', b'o', b'o'];
    from_bytes::<Option<String>>(buf, Some("foo".to_string()));
}

#[test]
fn test_parse_invalid_optional() {
    // the prefix has to be 0 or 1
    // 2 is invalid
    let buf: &[u8] = &[2, 0, 0, 0, 0];
    from_bytes_fail::<Option<u32>>(buf, Error::InvalidOptional);
}

#[test]
fn test_parse_true() {
    let buf: &[u8] = &[1];
    from_bytes::<bool>(buf, true);
}

#[test]
fn test_parse_false() {
    let buf: &[u8] = &[0];
    from_bytes::<bool>(buf, false);
}

#[test]
fn test_parse_invalid_bool() {
    // the bool has to be 0 or 1
    // 2 is invalid
    let buf: &[u8] = &[2];
    from_bytes_fail::<bool>(buf, Error::InvalidBool);
}

#[test]
fn test_parse_str() {
    let buf: &[u8] = &[0, 0, 0, 3, b'f', b'o', b'o'];
    from_bytes::<String>(buf, "foo".to_string());
}

#[test]
fn test_parse_invalid_utf8_str() {
    let buf: &[u8] = &[
        0, 0, 0, 11, 195, 165, 195, 0, 164, 195, 182, 195, 188, 195, 174,
    ];
    from_bytes_fail::<String>(buf, Error::InvalidString);
}

#[test]
fn test_parse_empty_str() {
    let buf: &[u8] = &[0, 0, 0, 0];
    from_bytes::<String>(buf, "".to_string());
}

#[test]
fn test_parse_truncated_str() {
    let buf: &[u8] = &[0, 0, 0, 10, b'f', b'o', b'o'];
    from_bytes_fail::<String>(buf, Error::EndOfBuffer);
}

#[cfg(test)]
use chia_streamable_macro::Streamable;

#[cfg(test)]
use crate::chia_error;

#[cfg(test)]
#[derive(Streamable, PartialEq, Debug)]
struct TestStruct {
    a: Vec<i8>,
    b: String,
    c: (u32, u32),
}

#[test]
fn test_parse_struct() {
    let buf: &[u8] = &[
        0, 0, 0, 2, 42, 0xff, 0, 0, 0, 3, b'b', b'a', b'z', 0xff, 0xff, 0xff, 0xff, 0, 0, 0x13,
        0x37,
    ];
    from_bytes::<TestStruct>(
        buf,
        TestStruct {
            a: vec![42_i8, -1],
            b: "baz".to_string(),
            c: (0xffffffff, 0x1337),
        },
    );
}

#[cfg(test)]
#[derive(Streamable, PartialEq, Debug)]
struct TestTuple(String, u32);

#[test]
fn test_parse_custom_tuple() {
    let buf: &[u8] = &[0, 0, 0, 3, b'b', b'a', b'z', 0, 0, 0, 42];
    from_bytes::<TestTuple>(buf, TestTuple("baz".to_string(), 42));
}

#[cfg(test)]
fn stream<T: Streamable>(v: &T) -> Vec<u8> {
    let mut buf = Vec::<u8>::new();
    v.stream(&mut buf).unwrap();
    let mut ctx1 = Sha256::new();
    let mut ctx2 = Sha256::new();
    v.update_digest(&mut ctx1);
    ctx2.update(&buf);
    assert_eq!(&ctx1.finalize(), &ctx2.finalize());
    buf
}

#[test]
fn test_stream_i32() {
    let b: i32 = 0x01020304;
    let buf = stream(&b);
    assert_eq!(&buf[..], [1, 2, 3, 4]);
}

#[test]
fn test_stream_sequence() {
    let b: Vec<u8> = vec![1, 2, 3, 4, 5, 42, 127];
    let buf = stream(&b);
    // 4 byte length prefix
    assert_eq!(&buf[..], [0, 0, 0, 7, 1, 2, 3, 4, 5, 42, 127]);
}

#[test]
fn test_stream_empty_sequence() {
    let b: Vec<u8> = vec![];
    let buf = stream(&b);
    // 4 byte length prefix
    assert_eq!(&buf[..], [0, 0, 0, 0]);
}

#[test]
fn test_stream_none() {
    let b: Option<u8> = None;
    let buf = stream(&b);
    assert_eq!(&buf[..], [0]);
}

#[test]
fn test_stream_optional() {
    let b: Option<u32> = Some(0x1337);
    let buf = stream(&b);
    assert_eq!(&buf[..], [1, 0, 0, 0x13, 0x37]);
}

#[test]
fn test_stream_optional_zero() {
    let b: Option<u32> = Some(0);
    let buf = stream(&b);
    assert_eq!(&buf[..], [1, 0, 0, 0, 0]);
}

#[test]
fn test_stream_optional_set1() {
    let out = stream(&Some(42_u32));
    assert_eq!(&out, &[1, 0, 0, 0, 42]);
}

#[test]
fn test_stream_optional_set2() {
    let out = stream(&Some("foobar".to_string()));
    assert_eq!(&out, &[1, 0, 0, 0, 6, b'f', b'o', b'o', b'b', b'a', b'r']);
}

#[test]
fn test_stream_tuple() {
    let b: (u8, u32, u64, bool) = (42, 0x1337, 0x0102030405060708, true);
    let buf = stream(&b);
    assert_eq!(&buf[..], [42, 0, 0, 0x13, 0x37, 1, 2, 3, 4, 5, 6, 7, 8, 1]);
}

#[test]
fn test_stream_tuple_of_lists() {
    let b: (Vec<u8>, Vec<u8>) = (vec![0, 1, 2, 3], vec![4, 5, 6, 7, 8, 9]);
    let buf = stream(&b);
    assert_eq!(
        &buf[..],
        [0, 0, 0, 4, 0, 1, 2, 3, 0, 0, 0, 6, 4, 5, 6, 7, 8, 9]
    );
}

#[test]
fn test_stream_tuple1() {
    let out = stream(&(42_u32));
    assert_eq!(&out, &[0, 0, 0, 42]);
}
#[test]
fn test_stream_tuple2() {
    let out = stream(&("test".to_string(), 42_u32));
    assert_eq!(&out, &[0, 0, 0, 4, b't', b'e', b's', b't', 0, 0, 0, 42]);
}

#[test]
fn test_stream_tuple_of_tuples() {
    let out = stream(&((0x1337_u32, 42_u32), ("foo".to_string(), "bar".to_string())));
    assert_eq!(
        &out,
        &[
            0, 0, 0x13, 0x37, 0, 0, 0, 42, 0, 0, 0, 3, b'f', b'o', b'o', 0, 0, 0, 3, b'b', b'a',
            b'r'
        ]
    );
}

#[test]
fn test_stream_false() {
    let b = false;
    let buf = stream(&b);
    assert_eq!(&buf[..], [0]);
}

#[test]
fn test_stream_true() {
    let b = true;
    let buf = stream(&b);
    assert_eq!(&buf[..], [1]);
}

#[test]
fn test_stream_string() {
    let b = "abc".to_string();
    let buf = stream(&b);
    assert_eq!(&buf[..], [0, 0, 0, 3, b'a', b'b', b'c']);
}

#[test]
fn test_stream_empty_string() {
    let b = "".to_string();
    let buf = stream(&b);
    assert_eq!(&buf[..], [0, 0, 0, 0]);
}

#[test]
fn test_stream_utf8_string() {
    let b = "åäöüî".to_string();
    let buf = stream(&b);
    assert_eq!(
        &buf[..],
        [0, 0, 0, 10, 195, 165, 195, 164, 195, 182, 195, 188, 195, 174]
    );
}

#[test]
fn test_stream_struct() {
    let b = TestStruct {
        a: [1, 2, 3].to_vec(),
        b: "abc".to_string(),
        c: (0x1337, 42),
    };
    let buf = stream(&b);
    assert_eq!(
        &buf[..],
        [0, 0, 0, 3, 1, 2, 3, 0, 0, 0, 3, b'a', b'b', b'c', 0, 0, 0x13, 0x37, 0, 0, 0, 42]
    );
}

#[test]
fn test_stream_custom_tuple() {
    let b = TestTuple("abc".to_string(), 1337);
    let buf = stream(&b);
    assert_eq!(&buf[..], [0, 0, 0, 3, b'a', b'b', b'c', 0, 0, 0x05, 0x39]);
}

#[test]
fn test_stream_bytes32() {
    let buf: &[u8] = &[
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32,
    ];
    let out = stream(&Bytes32::from(buf));
    assert_eq!(&buf, &out);
}

#[test]
fn test_stream_bytes() {
    let val: Bytes = vec![
        1_u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31, 32,
    ]
    .into();
    println!("{:?}", val);
    let buf = stream(&val);
    println!("buf: {:?}", buf);
    from_bytes(&buf, val);
}

#[test]
fn test_stream_list() {
    let out = stream(&vec![0x1030307_u32, 42, 0xffffffff]);
    assert_eq!(
        &out,
        &[0, 0, 0, 3, 1, 3, 3, 7, 0, 0, 0, 42, 0xff, 0xff, 0xff, 0xff]
    );
}

#[test]
fn test_stream_list_list() {
    let out = stream(&vec![
        vec![0x1030307_u32],
        vec![42_u32],
        vec![0xffffffff_u32],
    ]);
    assert_eq!(
        &out,
        &[
            0, 0, 0, 3, 0, 0, 0, 1, 1, 3, 3, 7, 0, 0, 0, 1, 0, 0, 0, 42, 0, 0, 0, 1, 0xff, 0xff,
            0xff, 0xff
        ]
    );
}

#[test]
fn test_stream_u128() {
    let out = stream(&(1337_u128, -1337_i128));
    assert_eq!(
        &out,
        &[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05, 0x39, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfa, 0xc7
        ]
    );
}

#[cfg(test)]
#[derive(Streamable, Hash, Copy, Debug, Clone, Eq, PartialEq)]
enum TestEnum {
    A = 0,
    B = 1,
    C = 255,
}

#[test]
fn test_parse_enum() {
    from_bytes::<TestEnum>(&[0], TestEnum::A);
    from_bytes::<TestEnum>(&[1], TestEnum::B);
    from_bytes::<TestEnum>(&[255], TestEnum::C);
    from_bytes_fail::<TestEnum>(&[3], Error::InvalidEnum);
    from_bytes_fail::<TestEnum>(&[254], Error::InvalidEnum);
    from_bytes_fail::<TestEnum>(&[128], Error::InvalidEnum);
}

#[test]
fn test_stream_enum() {
    assert_eq!(stream::<TestEnum>(&TestEnum::A), &[0_u8]);
    assert_eq!(stream::<TestEnum>(&TestEnum::B), &[1_u8]);
    assert_eq!(stream::<TestEnum>(&TestEnum::C), &[255_u8]);
}
