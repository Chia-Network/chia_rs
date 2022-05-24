use super::error::{Error, Result};
use std::io;

// A chia "Streamable" Serializer into a Writer. The Chia Streamable format is
// similar to bincode, but simpler in some respects. Like bincode, types are not
// encoded in the output stream, they are all expected to be known ahead of time
// (compile time in our case). Types are serialized like this:

// * fixed width primitive integer types are encoded as big endian.
// * fixed sized byte buffers are stored verbatim. Their size is known ahead of
//   time (e.g. Bytes32). BLS keys are also treated as fixed size buffers
//   (Bytes48 and Bytes96)
// * booleans are serialized as a single byte with value 0 or 1. Any other value
//   for booleans are considered invalid when deserializing
// * lists of variable length (but with all elements of the same type) are
//   encoded with a 32 bit, big endian, length-prefix followed by that many
//   elements.
// * variable length byte buffers are encoded as a list of bytes (see point
//   above).
// * strings are encoded as a list of bytes. Those bytes are the UTF-8 encoding
//   of the characters in the string. An invalid UTF-8 sequence is considered
//   invalid. Note that the length prefix denotes the number of bytes, not
//   characters.
// * an optional value is encoded as a byte prefix of value 1 followed by the
//   serialisation of the value, when the optional is engaged. A disengaged
//   optional (None) is encoded as a single byte 0. Any value other than 0 or 1
//   in the optional prefix is considered an error.
// * tuples and structs/classes are encoded simply encoded as all their members

// Some types are not supported by the Chia Streamable format. Notably:
// * dictionaries
// * floating point values
// * characters (since they aren't necessarily fixed width)
// * enums. They have to be converted to their underlying integer representation first
// * variants

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

macro_rules! serialize_not_supported {
    ($name:ident, $ret:ty $(, $arg:ident : $t:ty)*) => {
        fn $name(self $(, $arg : $t)*) -> Result<$ret> {
            Err(Error::NotSupported)
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

    serialize_not_supported!(serialize_unit, ());
    serialize_not_supported!(serialize_unit_struct, (), _name: &'static str);
    serialize_not_supported!(serialize_f32, (), _v: f32);
    serialize_not_supported!(serialize_f64, (), _v: f64);
    serialize_not_supported!(serialize_char, (), _c: char);
    serialize_not_supported!(serialize_map, Self::SerializeMap, _len: Option<usize>);
    serialize_not_supported!(
        serialize_tuple_variant,
        Self::SerializeTupleVariant,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize
    );
    serialize_not_supported!(
        serialize_struct_variant,
        Self::SerializeStructVariant,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize
    );
    serialize_not_supported!(
        serialize_unit_variant,
        (),
        _name: &'static str,
        _index: u32,
        _variant: &'static str
    );

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

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Ok(self)
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
    fn serialize_field<T: ?Sized>(&mut self, _value: &T) -> Result<()>
    where
        T: serde::ser::Serialize,
    {
        panic!("should not get here");
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
    fn serialize_field<T: ?Sized>(&mut self, _key: &'static str, _value: &T) -> Result<()>
    where
        T: serde::ser::Serialize,
    {
        panic!("should not get here");
    }
);
#[rustfmt::skip]
serialize_impl!(
    serde::ser::SerializeMap,
    fn serialize_key<K: ?Sized>(&mut self, _value: &K) -> Result<()>
    where
        K: serde::ser::Serialize,
    {
        panic!("should not get here");
    }
    fn serialize_value<V: ?Sized>(&mut self, _value: &V) -> Result<()>
    where
        V: serde::ser::Serialize,
    {
        panic!("should not get here");
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

#[cfg(test)]
fn stream_failure<T: serde::Serialize>(v: &T) -> Error {
    let mut buf = Vec::<u8>::new();
    let mut ser = ChiaSerializer::new(&mut buf);
    serde::Serialize::serialize(&v, &mut ser).unwrap_err()
}

#[test]
fn test_bytes() {
    let b: BytesImpl<32> = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31,
    ]
    .into();
    let buf = stream(&b);
    assert_eq!(&buf[..], b.as_ref());
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

#[cfg(test)]
#[derive(Serialize)]
struct NewTypeStruct(u8, i8);

#[test]
fn test_newtype_struct() {
    let out = stream(&NewTypeStruct(10, -1));
    assert_eq!(out, &[10, 0xff]);
}

#[test]
fn test_float() {
    assert_eq!(stream_failure(&3.14f32), Error::NotSupported);
    assert_eq!(stream_failure(&3.14f64), Error::NotSupported);
    assert_eq!(stream_failure(&(3.14f64, 10_u8)), Error::NotSupported);
    assert_eq!(stream_failure(&[3.14f32, 1.2345f32]), Error::NotSupported);
}

#[test]
fn test_char() {
    let c = 'ä';
    assert_eq!(stream_failure(&c), Error::NotSupported);
}

#[cfg(test)]
use std::collections::HashMap;

#[test]
fn test_hash_map() {
    let m = HashMap::from([("foo", 0), ("bar", 1)]);
    assert_eq!(stream_failure(&m), Error::NotSupported);
}

#[cfg(test)]
#[derive(Serialize)]
enum TestVariant {
    A(u8),
    B,
    C(String),
}

#[test]
fn test_variant() {
    assert_eq!(stream_failure(&TestVariant::A(5)), Error::NotSupported);
    assert_eq!(stream_failure(&TestVariant::B), Error::NotSupported);
    assert_eq!(
        stream_failure(&TestVariant::C("foobar".to_string())),
        Error::NotSupported
    );
}

#[cfg(test)]
#[derive(Serialize)]
enum TestEnum {
    A,
    B,
    C,
}

#[test]
fn test_enum() {
    assert_eq!(stream_failure(&TestEnum::A), Error::NotSupported);
    assert_eq!(stream_failure(&TestEnum::B), Error::NotSupported);
    assert_eq!(stream_failure(&TestEnum::C), Error::NotSupported);
}

#[cfg(test)]
#[derive(Serialize)]
struct UnitStruct;

#[test]
fn test_unit_struct() {
    assert_eq!(stream_failure(&UnitStruct {}), Error::NotSupported);
}

#[test]
fn test_unit() {
    let a = ();
    assert_eq!(stream_failure(&a), Error::NotSupported);
}
