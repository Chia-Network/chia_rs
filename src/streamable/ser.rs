use super::error::{Error, Result};
use std::io;

// A chia "Streamable" Serializer into a Writer.
pub struct ChiaSerializer<W: io::Write> {
    sink: W,
}

impl<W: io::Write> ChiaSerializer<W> {
    pub fn new(w: W) -> ChiaSerializer<W> {
        ChiaSerializer { sink: w }
    }
}

macro_rules! serialize_primitive {
    ($name:ident, $t:ty) => {
        fn $name(self, v: $t) -> Result<()> {
            self.sink.write_all(&v.to_be_bytes())?;
            Ok(())
        }
    };
}

impl<'a, W: io::Write> serde::Serializer for &'a mut ChiaSerializer<W> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Self;
    type SerializeMap = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_unit(self) -> Result<()> {
        Err(Error::NotSupported)
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<()> {
        Err(Error::NotSupported)
    }

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.sink.write_all(&[v as u8])?;
        Ok(())
    }

    serialize_primitive!(serialize_i8, i8);
    serialize_primitive!(serialize_u8, u8);
    serialize_primitive!(serialize_i16, i16);
    serialize_primitive!(serialize_u16, u16);
    serialize_primitive!(serialize_i32, i32);
    serialize_primitive!(serialize_u32, u32);
    serialize_primitive!(serialize_i64, i64);
    serialize_primitive!(serialize_u64, u64);

    fn serialize_f32(self, _v: f32) -> Result<()> {
        Err(Error::NotSupported)
    }
    fn serialize_f64(self, _v: f64) -> Result<()> {
        Err(Error::NotSupported)
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        // bytes is the UTF-8 sequence
        let bytes = v.bytes();
        self.serialize_u32(bytes.len() as u32)?;
        // since bytes is an iterator, we can't use self.sink.write_all()
        for b in bytes {
            self.serialize_u8(b)?;
        }
        Ok(())
    }

    fn serialize_char(self, _c: char) -> Result<()> {
        Err(Error::NotSupported)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.sink.write_all(v)?;
        Ok(())
    }

    fn serialize_none(self) -> Result<()> {
        self.sink.write_all(&[0])?;
        Ok(())
    }

    fn serialize_some<T: ?Sized>(self, v: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        self.sink.write_all(&[1])?;
        v.serialize(self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        match len {
            None => Err(Error::NotSupported),
            Some(len) => {
                // ERROR check cast to u32 and fail properly
                self.serialize_u32(len as u32)?;
                Ok(self)
            }
        }
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Ok(self)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Ok(self)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        Err(Error::NotSupported)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Err(Error::NotSupported)
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        Err(Error::NotSupported)
    }

    fn serialize_newtype_struct<T: ?Sized>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: serde::ser::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()>
    where
        T: serde::ser::Serialize,
    {
        Err(Error::NotSupported)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        Err(Error::NotSupported)
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

macro_rules! serialize_impl {
    ($t:ty, $($funs:tt)*) => {
impl<'a, W> $t for &'a mut ChiaSerializer<W>
where
    W: io::Write,
{
    type Ok = ();
    type Error = Error;

    $($funs)*

    fn end(self) -> Result<()> {
        Ok(())
    }
}

    }
}

serialize_impl!(
    serde::ser::SerializeSeq,
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }
);
serialize_impl!(
    serde::ser::SerializeTuple,
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }
);
serialize_impl!(
    serde::ser::SerializeTupleStruct,
    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }
);
serialize_impl!(
    serde::ser::SerializeTupleVariant,
    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<()>
    where
        T: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }
);
serialize_impl!(
    serde::ser::SerializeStruct,
    fn serialize_field<T: ?Sized>(&mut self, _key: &'static str, value: &T) -> Result<()>
    where
        T: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }
);
serialize_impl!(
    serde::ser::SerializeStructVariant,
    fn serialize_field<T: ?Sized>(&mut self, _key: &'static str, value: &T) -> Result<()>
    where
        T: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }
);
#[rustfmt::skip]
serialize_impl!(
    serde::ser::SerializeMap,
    fn serialize_key<K: ?Sized>(&mut self, value: &K) -> Result<()>
    where
        K: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }
    fn serialize_value<V: ?Sized>(&mut self, value: &V) -> Result<()>
    where
        V: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }
);

// ===== TESTS ====

#[cfg(test)]
use crate::streamable::bytes::BytesImpl;

#[cfg(test)]
use serde::Serialize;

#[cfg(test)]
fn stream<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut buf = Vec::<u8>::new();
    let mut ser = ChiaSerializer::new(&mut buf);
    serde::Serialize::serialize(&v, &mut ser).unwrap();
    buf
}

#[test]
fn test_bytes() {
    let b: BytesImpl<32> = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31,
    ]
    .into();
    let buf = stream(&b);
    assert_eq!(&buf[..], b.slice());
}

#[test]
fn test_i32() {
    let b: i32 = 0x01020304;
    let buf = stream(&b);
    assert_eq!(&buf[..], [1, 2, 3, 4]);
}

#[test]
fn test_sequence() {
    let b: Vec<u8> = vec![1, 2, 3, 4, 5, 42, 127];
    let buf = stream(&b);
    // 4 byte length prefix
    assert_eq!(&buf[..], [0, 0, 0, 7, 1, 2, 3, 4, 5, 42, 127]);
}

#[test]
fn test_empty_sequence() {
    let b: Vec<u8> = vec![];
    let buf = stream(&b);
    // 4 byte length prefix
    assert_eq!(&buf[..], [0, 0, 0, 0]);
}

#[test]
fn test_none() {
    let b: Option<u8> = None;
    let buf = stream(&b);
    assert_eq!(&buf[..], [0]);
}

#[test]
fn test_optional() {
    let b: Option<u32> = Some(0x1337);
    let buf = stream(&b);
    assert_eq!(&buf[..], [1, 0, 0, 0x13, 0x37]);
}

#[test]
fn test_optional_zero() {
    let b: Option<u32> = Some(0);
    let buf = stream(&b);
    assert_eq!(&buf[..], [1, 0, 0, 0, 0]);
}

#[test]
fn test_optional_set1() {
    let out = stream(&Some(42_u32));
    assert_eq!(&out, &[1, 0, 0, 0, 42]);
}

#[test]
fn test_optional_set2() {
    let out = stream(&Some("foobar"));
    assert_eq!(&out, &[1, 0, 0, 0, 6, b'f', b'o', b'o', b'b', b'a', b'r']);
}

#[test]
fn test_tuple() {
    let b: (u8, u32, u64, bool) = (42, 0x1337, 0x0102030405060708, true);
    let buf = stream(&b);
    assert_eq!(&buf[..], [42, 0, 0, 0x13, 0x37, 1, 2, 3, 4, 5, 6, 7, 8, 1]);
}

#[test]
fn test_tuple_of_lists() {
    let b: (Vec<u8>, Vec<u8>) = (vec![0, 1, 2, 3], vec![4, 5, 6, 7, 8, 9]);
    let buf = stream(&b);
    assert_eq!(
        &buf[..],
        [0, 0, 0, 4, 0, 1, 2, 3, 0, 0, 0, 6, 4, 5, 6, 7, 8, 9]
    );
}

#[test]
fn test_tuple1() {
    let out = stream(&(42_u32));
    assert_eq!(&out, &[0, 0, 0, 42]);
}
#[test]
fn test_tuple2() {
    let out = stream(&("test", 42_u32));
    assert_eq!(&out, &[0, 0, 0, 4, b't', b'e', b's', b't', 0, 0, 0, 42]);
}

#[test]
fn test_tuple_of_tuples() {
    let out = stream(&((0x1337_u32, 42_u32), ("foo", "bar")));
    assert_eq!(
        &out,
        &[
            0, 0, 0x13, 0x37, 0, 0, 0, 42, 0, 0, 0, 3, b'f', b'o', b'o', 0, 0, 0, 3, b'b', b'a',
            b'r'
        ]
    );
}

#[test]
fn test_false() {
    let b = false;
    let buf = stream(&b);
    assert_eq!(&buf[..], [0]);
}

#[test]
fn test_true() {
    let b = true;
    let buf = stream(&b);
    assert_eq!(&buf[..], [1]);
}

#[test]
fn test_string() {
    let b = "abc".to_string();
    let buf = stream(&b);
    assert_eq!(&buf[..], [0, 0, 0, 3, b'a', b'b', b'c']);
}

#[test]
fn test_empty_string() {
    let b = "".to_string();
    let buf = stream(&b);
    assert_eq!(&buf[..], [0, 0, 0, 0]);
}

#[test]
fn test_utf8_string() {
    let b = "åäöüî".to_string();
    let buf = stream(&b);
    assert_eq!(
        &buf[..],
        [0, 0, 0, 10, 195, 165, 195, 164, 195, 182, 195, 188, 195, 174]
    );
}

#[cfg(test)]
#[derive(Serialize)]
struct TestStruct {
    a: Vec<u8>,
    b: String,
    c: (u32, u32),
}

#[test]
fn test_struct() {
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

#[cfg(test)]
use crate::streamable::bytes::Bytes32;

#[test]
fn test_bytes32() {
    let buf: &[u8] = &[
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32,
    ];
    let out = stream(&Bytes32::from(buf));
    assert_eq!(&buf, &out);
}

#[test]
fn test_list() {
    let out = stream(&vec![0x1030307_u32, 42, 0xffffffff]);
    assert_eq!(
        &out,
        &[0, 0, 0, 3, 1, 3, 3, 7, 0, 0, 0, 42, 0xff, 0xff, 0xff, 0xff]
    );
}

#[test]
fn test_list_list() {
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
