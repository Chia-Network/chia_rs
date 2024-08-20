use core::str;

use serde::{de, Deserialize};

use crate::{Error, Result};

pub struct Deserializer<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

pub fn from_bytes<'a, T>(bytes: &'a [u8]) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer { bytes, cursor: 0 };
    T::deserialize(&mut deserializer)
}

pub fn from_bytes_exact<'a, T>(bytes: &'a [u8]) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer { bytes, cursor: 0 };
    let value = T::deserialize(&mut deserializer)?;
    if deserializer.cursor >= deserializer.bytes.len() {
        Ok(value)
    } else {
        Err(Error::ExpectedEof)
    }
}

impl<'a> Deserializer<'a> {
    fn eat<const LEN: usize>(&mut self) -> Result<[u8; LEN]> {
        if self.cursor + LEN > self.bytes.len() {
            return Err(Error::UnexpectedEof);
        }
        let mut bytes = [0; LEN];
        bytes.copy_from_slice(&self.bytes[self.cursor..self.cursor + LEN]);
        self.cursor += LEN;
        Ok(bytes)
    }

    fn eat_slice(&mut self, len: usize) -> Result<&'a [u8]> {
        if self.cursor + len > self.bytes.len() {
            return Err(Error::UnexpectedEof);
        }
        let slice = &self.bytes[self.cursor..self.cursor + len];
        self.cursor += len;
        Ok(slice)
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn is_human_readable(&self) -> bool {
        false
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        if self.cursor >= self.bytes.len() {
            return Err(Error::UnexpectedEof);
        }
        let value = self.bytes[self.cursor];
        self.cursor += 1;
        visitor.visit_u8(value)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.eat::<2>()?;
        let value = u16::from_be_bytes(bytes);
        visitor.visit_u16(value)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.eat::<4>()?;
        let value = u32::from_be_bytes(bytes);
        visitor.visit_u32(value)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.eat::<8>()?;
        let value = u64::from_be_bytes(bytes);
        visitor.visit_u64(value)
    }

    fn deserialize_u128<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.eat::<16>()?;
        let value = u128::from_be_bytes(bytes);
        visitor.visit_u128(value)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        if self.cursor >= self.bytes.len() {
            return Err(Error::UnexpectedEof);
        }
        let value = self.bytes[self.cursor] as i8;
        self.cursor += 1;
        visitor.visit_i8(value)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.eat::<2>()?;
        let value = i16::from_be_bytes(bytes);
        visitor.visit_i16(value)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.eat::<4>()?;
        let value = i32::from_be_bytes(bytes);
        visitor.visit_i32(value)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.eat::<8>()?;
        let value = i64::from_be_bytes(bytes);
        visitor.visit_i64(value)
    }

    fn deserialize_i128<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.eat::<16>()?;
        let value = i128::from_be_bytes(bytes);
        visitor.visit_i128(value)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.eat::<4>()?;
        let value = f32::from_be_bytes(bytes);
        visitor.visit_f32(value)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let bytes = self.eat::<8>()?;
        let value = f64::from_be_bytes(bytes);
        visitor.visit_f64(value)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        if self.cursor >= self.bytes.len() {
            return Err(Error::UnexpectedEof);
        }
        let value = self.bytes[self.cursor];
        self.cursor += 1;
        visitor.visit_bool(match value {
            0 => false,
            1 => true,
            _ => return Err(Error::UnexpectedBool(value)),
        })
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let len = u32::deserialize(&mut *self)?;
        let bytes = self.eat_slice(len as usize)?;
        let value = String::from_utf8(bytes.to_vec())?;
        visitor.visit_string(value)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let len = u32::deserialize(&mut *self)?;
        let bytes = self.eat_slice(len as usize)?;
        let value = str::from_utf8(bytes)?;
        visitor.visit_str(value)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let len = u32::deserialize(&mut *self)?;
        let bytes = self.eat_slice(len as usize)?;
        let Some(value) = str::from_utf8(bytes)?.chars().next() else {
            return Err(Error::MissingChar);
        };
        visitor.visit_char(value)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        if self.cursor >= self.bytes.len() {
            return Err(Error::UnexpectedEof);
        }

        let value = self.bytes[self.cursor];
        self.cursor += 1;
        let is_some = match value {
            0 => false,
            1 => true,
            _ => return Err(Error::UnexpectedOptionalInt(value)),
        };

        if is_some {
            visitor.visit_some(self)
        } else {
            visitor.visit_none()
        }
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let len = u32::deserialize(&mut *self)?;
        let bytes = self.eat_slice(len as usize)?;
        visitor.visit_bytes(bytes)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let len = u32::deserialize(&mut *self)?;
        let bytes = self.eat_slice(len as usize)?;
        visitor.visit_byte_buf(bytes.to_vec())
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let len = u32::deserialize(&mut *self)?;
        visitor.visit_seq(SeqAccess {
            deserializer: self,
            len,
        })
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(SeqAccess {
            deserializer: self,
            len: len as u32,
        })
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(SeqAccess {
            deserializer: self,
            len: len as u32,
        })
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(SeqAccess {
            deserializer: self,
            len: fields.len() as u32,
        })
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::DeserializeAny)
    }

    fn deserialize_map<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::Map)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::Enum)
    }

    fn deserialize_identifier<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::Identifier)
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::DeserializeAny)
    }
}

struct SeqAccess<'a, 'de> {
    deserializer: &'a mut Deserializer<'de>,
    len: u32,
}

impl<'a, 'de> de::SeqAccess<'de> for SeqAccess<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        if self.len == 0 {
            return Ok(None);
        }
        self.len -= 1;
        seed.deserialize(&mut *self.deserializer).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use de::DeserializeOwned;

    use super::*;

    #[derive(PartialEq, Debug, Deserialize)]
    struct TestStruct {
        a: Vec<i8>,
        b: String,
        c: (u32, u32),
    }

    #[derive(PartialEq, Debug, Deserialize)]
    struct TestTuple(String, u32);

    #[allow(clippy::needless_pass_by_value)]
    fn from_bytes<T: DeserializeOwned + PartialEq + fmt::Debug>(bytes: &[u8], expected: T) {
        let value = super::from_bytes::<T>(bytes).unwrap();
        assert_eq!(value, expected);
    }

    fn from_bytes_fail<T: DeserializeOwned + PartialEq + fmt::Debug>(
        bytes: &[u8],
        expected: Error,
    ) {
        let value = super::from_bytes::<T>(bytes);
        assert_eq!(value, Err(expected));
    }

    fn from_bytes_fail_any<T: DeserializeOwned + fmt::Debug>(bytes: &[u8]) {
        let value = super::from_bytes::<T>(bytes);
        assert!(value.is_err());
    }

    #[test]
    fn test_parse_u64() {
        from_bytes::<u64>(&[0, 0, 0, 0, 0, 0, 0, 0], 0);
        from_bytes::<u64>(&[0, 0, 0, 0, 0, 0, 0, 1], 1);
        from_bytes::<u64>(&[0x80, 0, 0, 0, 0, 0, 0, 0], 0x8000_0000_0000_0000);
        from_bytes::<u64>(
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
            0xffff_ffff_ffff_ffff,
        );

        // truncated
        from_bytes_fail::<u64>(&[0, 0, 0, 0, 0, 0, 0], Error::UnexpectedEof);
        from_bytes_fail::<u64>(&[0, 0, 0, 0, 0, 0], Error::UnexpectedEof);
        from_bytes_fail::<u64>(&[0, 0, 0, 0, 0], Error::UnexpectedEof);
        from_bytes_fail::<u64>(&[0, 0, 0, 0], Error::UnexpectedEof);
        from_bytes_fail::<u64>(&[0, 0, 0], Error::UnexpectedEof);
        from_bytes_fail::<u64>(&[0, 0], Error::UnexpectedEof);
    }

    #[test]
    fn test_parse_u128() {
        from_bytes::<u128>(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 0);
        from_bytes::<u128>(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1], 1);
        from_bytes::<u128>(
            &[0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            0x8000_0000_0000_0000_0000_0000_0000_0000,
        );
        from_bytes::<u128>(
            &[
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff,
            ],
            0xffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff,
        );

        // truncated
        from_bytes_fail::<u128>(
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            Error::UnexpectedEof,
        );
        from_bytes_fail::<u128>(
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            Error::UnexpectedEof,
        );
        from_bytes_fail::<u128>(
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            Error::UnexpectedEof,
        );
        from_bytes_fail::<u128>(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], Error::UnexpectedEof);
        from_bytes_fail::<u128>(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], Error::UnexpectedEof);
        from_bytes_fail::<u128>(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0], Error::UnexpectedEof);
    }

    #[test]
    fn test_parse_empty_list() {
        let buf: &[u8] = &[0, 0, 0, 0];
        from_bytes::<Vec<u32>>(buf, vec![]);
    }

    #[test]
    fn test_parse_list_1() {
        let buf: &[u8] = &[0, 0, 0, 1, 1, 2, 3, 4];
        from_bytes::<Vec<u32>>(buf, vec![0x0102_0304]);
    }

    #[test]
    fn test_parse_list_3() {
        let buf: &[u8] = &[0, 0, 0, 3, 1, 2, 3, 4, 1, 3, 3, 7, 0, 0, 4, 2];
        from_bytes::<Vec<u32>>(buf, vec![0x0102_0304, 0x0103_0307, 0x402]);
    }

    #[test]
    fn test_parse_list_list_3() {
        let buf: &[u8] = &[
            0, 0, 0, 3, 0, 0, 0, 1, 1, 2, 3, 4, 0, 0, 0, 1, 1, 3, 3, 7, 0, 0, 0, 1, 0, 0, 4, 2,
        ];
        from_bytes::<Vec<Vec<u32>>>(buf, vec![vec![0x0102_0304], vec![0x0103_0307], vec![0x402]]);
    }

    #[test]
    fn test_parse_list_empty() {
        let buf: &[u8] = &[0, 0, 0, 3];
        from_bytes::<Vec<()>>(buf, vec![(), (), ()]);
    }

    #[test]
    fn test_parse_long_list() {
        let buf: &[u8] = &[0xff, 0xff, 0xff, 0xff, 0, 0, 0];
        from_bytes_fail::<Vec<u32>>(buf, Error::UnexpectedEof);
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
        from_bytes_fail::<Option<u32>>(buf, Error::UnexpectedOptionalInt(2));
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
        from_bytes_fail::<bool>(buf, Error::UnexpectedBool(2));
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
        from_bytes_fail_any::<String>(buf);
    }

    #[test]
    fn test_parse_empty_str() {
        let buf: &[u8] = &[0, 0, 0, 0];
        from_bytes::<String>(buf, String::new());
    }

    #[test]
    fn test_parse_truncated_str() {
        let buf: &[u8] = &[0, 0, 0, 10, b'f', b'o', b'o'];
        from_bytes_fail::<String>(buf, Error::UnexpectedEof);
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
                c: (0xffff_ffff, 0x1337),
            },
        );
    }

    #[test]
    fn test_parse_custom_tuple() {
        let buf: &[u8] = &[0, 0, 0, 3, b'b', b'a', b'z', 0, 0, 0, 42];
        from_bytes::<TestTuple>(buf, TestTuple("baz".to_string(), 42));
    }
}
