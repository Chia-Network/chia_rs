use clvmr::sha2::Sha256;
use serde::{ser, Serialize};

use crate::{Error, Result};

struct Serializer<T> {
    output: T,
}

trait Encoder {
    fn push(&mut self, byte: u8);
    fn extend_from_slice(&mut self, bytes: &[u8]);
    fn reserve(&mut self, len: usize);
}

impl Encoder for Vec<u8> {
    fn push(&mut self, byte: u8) {
        self.push(byte);
    }

    fn extend_from_slice(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }

    fn reserve(&mut self, len: usize) {
        self.reserve(len);
    }
}

impl Encoder for Sha256 {
    fn push(&mut self, byte: u8) {
        self.update([byte]);
    }

    fn extend_from_slice(&mut self, bytes: &[u8]) {
        self.update(bytes);
    }

    fn reserve(&mut self, _len: usize) {}
}

pub fn to_bytes<T>(value: &T) -> Result<Vec<u8>>
where
    T: Serialize,
{
    let mut serializer = Serializer { output: Vec::new() };
    value.serialize(&mut serializer)?;
    Ok(serializer.output)
}

pub fn hash<T>(value: &T) -> Result<[u8; 32]>
where
    T: Serialize,
{
    let mut serializer = Serializer {
        output: Sha256::new(),
    };
    value.serialize(&mut serializer)?;
    Ok(serializer.output.finalize())
}

impl<'a, E> ser::Serializer for &'a mut Serializer<E>
where
    E: Encoder,
{
    type Ok = ();
    type Error = Error;

    type SerializeStruct = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeSeq = Self;
    type SerializeStructVariant = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.output.push(u8::from(v));
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.output.push(v);
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.output.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.output.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        self.output.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_u128(self, v: u128) -> Result<()> {
        self.output.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.output.push(v as u8);
        Ok(())
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.output.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        self.output.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        self.output.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_i128(self, v: i128) -> Result<()> {
        self.output.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<()> {
        self.output.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        self.output.extend_from_slice(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<()> {
        v.to_string().serialize(self)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.output
            .extend_from_slice(&(v.len() as u32).to_be_bytes());
        self.output.extend_from_slice(v);
        Ok(())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.output
            .extend_from_slice(&(v.len() as u32).to_be_bytes());
        self.output.extend_from_slice(v.as_bytes());
        Ok(())
    }

    fn serialize_none(self) -> Result<()> {
        self.output.push(0);
        Ok(())
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.output.push(1);
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        Ok(())
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self> {
        Ok(self)
    }

    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<Self> {
        Ok(self)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self> {
        Ok(self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self> {
        let Some(len) = len else {
            return Err(Error::UnknownLength);
        };
        self.output.reserve(len + 4);
        self.output.extend_from_slice(&(len as u32).to_be_bytes());
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self> {
        Err(Error::Enum)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self> {
        Err(Error::Enum)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        Err(Error::Enum)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::Enum)
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self> {
        Err(Error::Map)
    }
}

impl<'a, E> ser::SerializeStruct for &'a mut Serializer<E>
where
    E: Encoder,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, E> ser::SerializeTuple for &'a mut Serializer<E>
where
    E: Encoder,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, E> ser::SerializeTupleStruct for &'a mut Serializer<E>
where
    E: Encoder,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, E> ser::SerializeSeq for &'a mut Serializer<E>
where
    E: Encoder,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, E> ser::SerializeStructVariant for &'a mut Serializer<E>
where
    E: Encoder,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::Enum)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, E> ser::SerializeTupleVariant for &'a mut Serializer<E>
where
    E: Encoder,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::Enum)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, E> ser::SerializeMap for &'a mut Serializer<E>
where
    E: Encoder,
{
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, _key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::Map)
    }

    fn serialize_value<T>(&mut self, _value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::Map)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct TestStruct {
        a: Vec<i8>,
        b: String,
        c: (u32, u32),
    }

    #[derive(Serialize)]
    struct TestTuple(String, u32);

    #[test]
    fn test_stream_i32() {
        let b: i32 = 0x0102_0304;
        let buf = to_bytes(&b).unwrap();
        assert_eq!(&buf[..], [1, 2, 3, 4]);
    }

    #[test]
    fn test_stream_sequence() {
        let b: Vec<u8> = vec![1, 2, 3, 4, 5, 42, 127];
        let buf = to_bytes(&b).unwrap();
        // 4 byte length prefix
        assert_eq!(&buf[..], [0, 0, 0, 7, 1, 2, 3, 4, 5, 42, 127]);
    }

    #[test]
    fn test_stream_empty_sequence() {
        let b: Vec<u8> = vec![];
        let buf = to_bytes(&b).unwrap();
        // 4 byte length prefix
        assert_eq!(&buf[..], [0, 0, 0, 0]);
    }

    #[test]
    fn test_stream_none() {
        let b: Option<u8> = None;
        let buf = to_bytes(&b).unwrap();
        assert_eq!(&buf[..], [0]);
    }

    #[test]
    fn test_stream_optional() {
        let b: Option<u32> = Some(0x1337);
        let buf = to_bytes(&b).unwrap();
        assert_eq!(&buf[..], [1, 0, 0, 0x13, 0x37]);
    }

    #[test]
    fn test_stream_optional_zero() {
        let b: Option<u32> = Some(0);
        let buf = to_bytes(&b).unwrap();
        assert_eq!(&buf[..], [1, 0, 0, 0, 0]);
    }

    #[test]
    fn test_stream_optional_set1() {
        let out = to_bytes(&Some(42_u32)).unwrap();
        assert_eq!(&out, &[1, 0, 0, 0, 42]);
    }

    #[test]
    fn test_stream_optional_set2() {
        let out = to_bytes(&Some("foobar".to_string())).unwrap();
        assert_eq!(&out, &[1, 0, 0, 0, 6, b'f', b'o', b'o', b'b', b'a', b'r']);
    }

    #[test]
    fn test_stream_tuple() {
        let b: (u8, u32, u64, bool) = (42, 0x1337, 0x0102_0304_0506_0708, true);
        let buf = to_bytes(&b).unwrap();
        assert_eq!(&buf[..], [42, 0, 0, 0x13, 0x37, 1, 2, 3, 4, 5, 6, 7, 8, 1]);
    }

    #[test]
    fn test_stream_tuple_of_lists() {
        let b: (Vec<u8>, Vec<u8>) = (vec![0, 1, 2, 3], vec![4, 5, 6, 7, 8, 9]);
        let buf = to_bytes(&b).unwrap();
        assert_eq!(
            &buf[..],
            [0, 0, 0, 4, 0, 1, 2, 3, 0, 0, 0, 6, 4, 5, 6, 7, 8, 9]
        );
    }

    #[test]
    fn test_stream_tuple1() {
        let out = to_bytes(&(42_u32)).unwrap();
        assert_eq!(&out, &[0, 0, 0, 42]);
    }
    #[test]
    fn test_stream_tuple2() {
        let out = to_bytes(&("test".to_string(), 42_u32)).unwrap();
        assert_eq!(&out, &[0, 0, 0, 4, b't', b'e', b's', b't', 0, 0, 0, 42]);
    }

    #[test]
    fn test_stream_tuple_of_tuples() {
        let out =
            to_bytes(&((0x1337_u32, 42_u32), ("foo".to_string(), "bar".to_string()))).unwrap();
        assert_eq!(
            &out,
            &[
                0, 0, 0x13, 0x37, 0, 0, 0, 42, 0, 0, 0, 3, b'f', b'o', b'o', 0, 0, 0, 3, b'b',
                b'a', b'r'
            ]
        );
    }

    #[test]
    fn test_stream_false() {
        let b = false;
        let buf = to_bytes(&b).unwrap();
        assert_eq!(&buf[..], [0]);
    }

    #[test]
    fn test_stream_true() {
        let b = true;
        let buf = to_bytes(&b).unwrap();
        assert_eq!(&buf[..], [1]);
    }

    #[test]
    fn test_stream_string() {
        let b = "abc".to_string();
        let buf = to_bytes(&b).unwrap();
        assert_eq!(&buf[..], [0, 0, 0, 3, b'a', b'b', b'c']);
    }

    #[test]
    fn test_stream_empty_string() {
        let b = String::new();
        let buf = to_bytes(&b).unwrap();
        assert_eq!(&buf[..], [0, 0, 0, 0]);
    }

    #[test]
    fn test_stream_utf8_string() {
        let b = "åäöüî".to_string();
        let buf = to_bytes(&b).unwrap();
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
        let buf = to_bytes(&b).unwrap();
        assert_eq!(
            &buf[..],
            [0, 0, 0, 3, 1, 2, 3, 0, 0, 0, 3, b'a', b'b', b'c', 0, 0, 0x13, 0x37, 0, 0, 0, 42]
        );
    }

    #[test]
    fn test_stream_custom_tuple() {
        let b = TestTuple("abc".to_string(), 1337);
        let buf = to_bytes(&b).unwrap();
        assert_eq!(&buf[..], [0, 0, 0, 3, b'a', b'b', b'c', 0, 0, 0x05, 0x39]);
    }

    #[test]
    fn test_stream_list() {
        let out = to_bytes(&vec![0x0103_0307_u32, 42, 0xffff_ffff]).unwrap();
        assert_eq!(
            &out,
            &[0, 0, 0, 3, 1, 3, 3, 7, 0, 0, 0, 42, 0xff, 0xff, 0xff, 0xff]
        );
    }

    #[test]
    fn test_stream_list_of_empty() {
        let out = to_bytes(&vec![(), (), ()]).unwrap();
        assert_eq!(&out, &[0, 0, 0, 3]);
    }

    #[test]
    fn test_stream_list_list() {
        let out = to_bytes(&vec![
            vec![0x0103_0307_u32],
            vec![42_u32],
            vec![0xffff_ffff_u32],
        ])
        .unwrap();
        assert_eq!(
            &out,
            &[
                0, 0, 0, 3, 0, 0, 0, 1, 1, 3, 3, 7, 0, 0, 0, 1, 0, 0, 0, 42, 0, 0, 0, 1, 0xff,
                0xff, 0xff, 0xff
            ]
        );
    }

    #[test]
    fn test_stream_u128() {
        let out = to_bytes(&(1337_u128, -1337_i128)).unwrap();
        assert_eq!(
            &out,
            &[
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x05, 0x39, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfa, 0xc7
            ]
        );
    }
}
