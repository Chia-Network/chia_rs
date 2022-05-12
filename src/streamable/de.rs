use super::error::{Error, Result};
use std::convert::TryInto;

// A Chia "Streamable" Deserializer.
// This deserializer is similar to the bincode format. See ser.rs for a full
// description of the Chia Streamable serialization format.
pub struct ChiaDeserializer<'storage> {
    buf: &'storage [u8],
    pos: u32,
}

impl<'de> ChiaDeserializer<'de> {
    pub fn from_slice(b: &'de [u8]) -> Result<Self> {
        if b.len() > u32::MAX as usize {
            Err(Error::InputTooLarge)
        } else {
            Ok(ChiaDeserializer { buf: b, pos: 0 })
        }
    }

    fn read_slice(&mut self, len: u32) -> Result<&'de [u8]> {
        if (self.buf.len() as u32) - self.pos < len {
            return Err(Error::EndOfBuffer);
        }
        let ret = &self.buf[(self.pos as usize)..((self.pos + len) as usize)];
        self.pos += len;
        Ok(ret)
    }

    fn buf_left(&self) -> usize {
        self.buf.len() - self.pos as usize
    }

    pub fn pos(&self) -> u32 {
        self.pos
    }

    fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_slice(1)?[0])
    }

    fn read_u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.read_slice(4)?.try_into().unwrap()))
    }
}

macro_rules! deserialize_primitive {
    ($name:ident, $visit_name:ident, $t:ty) => {
        fn $name<V>(self, visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
        {
            const SIZE: usize = core::mem::size_of::<$t>();
            if SIZE > (8 as usize) {
                return Err(Error::NotSupported);
            }
            let buf = self.read_slice(SIZE as u32)?;
            visitor.$visit_name(<$t>::from_be_bytes(buf.try_into().unwrap()))
        }
    };
}

macro_rules! deserialize_not_supported {
    ($name:ident) => {
        fn $name<V>(self, _visitor: V) -> Result<V::Value>
        where
            V: serde::de::Visitor<'de>,
        {
            Err(Error::NotSupported)
        }
    };
}

impl<'de, 'a> serde::Deserializer<'de> for &'a mut ChiaDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        match self.read_u8()? {
            1 => visitor.visit_bool(true),
            0 => visitor.visit_bool(false),
            _ => Err(Error::InvalidBool),
        }
    }

    deserialize_primitive!(deserialize_u16, visit_u16, u16);
    deserialize_primitive!(deserialize_i16, visit_i16, i16);
    deserialize_primitive!(deserialize_u32, visit_u32, u32);
    deserialize_primitive!(deserialize_i32, visit_i32, i32);
    deserialize_primitive!(deserialize_u64, visit_u64, u64);
    deserialize_primitive!(deserialize_i64, visit_i64, i64);

    deserialize_not_supported!(deserialize_f32);
    deserialize_not_supported!(deserialize_f64);

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_u8(self.read_u8()?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_i8(self.read_u8()? as i8)
    }

    fn deserialize_unit<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    deserialize_not_supported!(deserialize_char);
    deserialize_not_supported!(deserialize_str);

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        let len = self.read_u32()?;
        let s = String::from(
            std::str::from_utf8(self.read_slice(len)?).map_err(|_e| Error::InvalidString)?,
        );
        visitor.visit_string(s)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        let len = self.read_u32()?;
        visitor.visit_borrowed_bytes(self.read_slice(len)?)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        let len = self.read_u32()?;
        visitor.visit_borrowed_bytes(self.read_slice(len)?)
    }

    fn deserialize_enum<V>(
        self,
        _enum: &'static str,
        _variants: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        struct SequenceAccess<'de, 'a> {
            de: &'a mut ChiaDeserializer<'de>,
            items: usize,
        }

        impl<'de, 'a, 'b: 'a> serde::de::SeqAccess<'de> for SequenceAccess<'de, 'a> {
            type Error = Error;

            fn size_hint(&self) -> Option<usize> {
                Some(self.items)
            }

            fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
            where
                T: serde::de::DeserializeSeed<'de>,
            {
                if self.items == 0 {
                    Ok(None)
                } else {
                    self.items -= 1;
                    let value = serde::de::DeserializeSeed::deserialize(seed, &mut *self.de)?;
                    Ok(Some(value))
                }
            }
        }

        if len > u32::MAX as usize {
            return Err(Error::SequenceTooLarge);
        }
        if len > self.buf_left() {
            return Err(Error::EndOfBuffer);
        }
        visitor.visit_seq(SequenceAccess {
            de: self,
            items: len,
        })
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        match self.read_u8()? {
            0 => visitor.visit_none(),
            1 => visitor.visit_some(&mut *self),
            _ => Err(Error::InvalidOptional),
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        let len = self.read_u32()?;
        self.deserialize_tuple(len as usize, visitor)
    }

    deserialize_not_supported!(deserialize_map);

    fn deserialize_struct<V>(
        self,
        _name: &str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_tuple(fields.len(), visitor)
    }

    deserialize_not_supported!(deserialize_identifier);

    fn deserialize_newtype_struct<V>(self, _name: &str, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, _visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        Err(Error::NotSupported)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        self.deserialize_tuple(len, visitor)
    }

    deserialize_not_supported!(deserialize_ignored_any);

    fn is_human_readable(&self) -> bool {
        false
    }
}

impl<'de> serde::de::VariantAccess<'de> for &'de mut ChiaDeserializer<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: serde::de::DeserializeSeed<'de>,
    {
        serde::de::DeserializeSeed::deserialize(seed, self)
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        serde::de::Deserializer::deserialize_tuple(self, len, visitor)
    }

    fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: serde::de::Visitor<'de>,
    {
        serde::de::Deserializer::deserialize_tuple(self, fields.len(), visitor)
    }
}

// ===== TESTS ====

#[cfg(test)]
use crate::streamable::bytes::{Bytes32, Bytes48};

#[cfg(test)]
fn from_bytes<'de, T: serde::de::Deserialize<'de> + std::fmt::Debug + std::cmp::PartialEq>(
    buf: &'de [u8],
    expected: T,
) {
    let mut de = ChiaDeserializer::from_slice(buf).unwrap();
    assert_eq!(T::deserialize(&mut de).unwrap(), expected);
}

#[cfg(test)]
fn from_bytes_fail<'de, T: serde::de::Deserialize<'de> + std::fmt::Debug + std::cmp::PartialEq>(
    buf: &'de [u8],
    expected: Error,
) {
    let mut de = ChiaDeserializer::from_slice(buf).unwrap();
    assert_eq!(T::deserialize(&mut de).unwrap_err(), expected);
}

#[test]
fn test_u64() {
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
fn test_bytes32() {
    let buf: &[u8] = &[
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32,
    ];
    from_bytes::<Bytes32>(buf, Bytes32::from(buf));
    from_bytes_fail::<Bytes32>(&buf[0..30], Error::EndOfBuffer);
}

#[test]
fn test_bytes48() {
    let buf: &[u8] = &[
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
    ];
    from_bytes::<Bytes48>(buf, Bytes48::from(buf));
    from_bytes_fail::<Bytes48>(&buf[0..47], Error::EndOfBuffer);
}

#[test]
fn test_empty_list() {
    let buf: &[u8] = &[0, 0, 0, 0];
    from_bytes::<Vec<u32>>(buf, vec![]);
}

#[test]
fn test_list_1() {
    let buf: &[u8] = &[0, 0, 0, 1, 1, 2, 3, 4];
    from_bytes::<Vec<u32>>(buf, vec![0x01020304]);
}

#[test]
fn test_list_3() {
    let buf: &[u8] = &[0, 0, 0, 3, 1, 2, 3, 4, 1, 3, 3, 7, 0, 0, 4, 2];
    from_bytes::<Vec<u32>>(buf, vec![0x01020304, 0x01030307, 0x402]);
}

#[test]
fn test_list_list_3() {
    let buf: &[u8] = &[
        0, 0, 0, 3, 0, 0, 0, 1, 1, 2, 3, 4, 0, 0, 0, 1, 1, 3, 3, 7, 0, 0, 0, 1, 0, 0, 4, 2,
    ];
    from_bytes::<Vec<Vec<u32>>>(buf, vec![vec![0x01020304], vec![0x01030307], vec![0x402]]);
}

#[test]
fn test_long_list() {
    let buf: &[u8] = &[0xff, 0xff, 0xff, 0xff, 0, 0, 0];
    from_bytes_fail::<Vec<u32>>(buf, Error::EndOfBuffer);
}

#[test]
fn test_tuple() {
    let buf: &[u8] = &[0, 0, 0, 3, 42, 0xff];
    from_bytes::<(u32, u8, i8)>(buf, (3, 42, -1));
}

#[test]
fn test_nested_tuple() {
    let buf: &[u8] = &[0, 0, 0, 3, 42, 43, 44, 0xff, 0xff, 0xff, 0xff];
    from_bytes::<(u32, (u8, u8, u8), i32)>(buf, (3, (42, 43, 44), -1));
}

#[test]
fn test_optional_clear() {
    let buf: &[u8] = &[0];
    from_bytes::<Option<u32>>(buf, None);
}

#[test]
fn test_optional_zero() {
    let buf: &[u8] = &[1, 0];
    from_bytes::<Option<u8>>(buf, Some(0));
}

#[test]
fn test_optional_u32() {
    let buf: &[u8] = &[1, 0, 0, 0x13, 0x37];
    from_bytes::<Option<u32>>(buf, Some(0x1337));
}

#[test]
fn test_optional_str() {
    let buf: &[u8] = &[1, 0, 0, 0, 3, b'f', b'o', b'o'];
    from_bytes::<Option<String>>(buf, Some("foo".to_string()));
}

#[test]
fn test_invalid_optional() {
    // the prefix has to be 0 or 1
    // 2 is invalid
    let buf: &[u8] = &[2, 0, 0, 0, 0];
    from_bytes_fail::<Option<u32>>(buf, Error::InvalidOptional);
}

#[test]
fn test_true() {
    let buf: &[u8] = &[1];
    from_bytes::<bool>(buf, true);
}

#[test]
fn test_false() {
    let buf: &[u8] = &[0];
    from_bytes::<bool>(buf, false);
}

#[test]
fn test_invalid_bool() {
    // the bool has to be 0 or 1
    // 2 is invalid
    let buf: &[u8] = &[2];
    from_bytes_fail::<bool>(buf, Error::InvalidBool);
}

#[test]
fn test_str() {
    let buf: &[u8] = &[0, 0, 0, 3, b'f', b'o', b'o'];
    from_bytes::<String>(buf, "foo".to_string());
}

#[test]
fn test_invalid_utf8_str() {
    let buf: &[u8] = &[
        0, 0, 0, 11, 195, 165, 195, 0, 164, 195, 182, 195, 188, 195, 174,
    ];
    from_bytes_fail::<String>(buf, Error::InvalidString);
}

#[test]
fn test_empty_str() {
    let buf: &[u8] = &[0, 0, 0, 0];
    from_bytes::<String>(buf, "".to_string());
}

#[test]
fn test_truncated_str() {
    let buf: &[u8] = &[0, 0, 0, 10, b'f', b'o', b'o'];
    from_bytes_fail::<String>(buf, Error::EndOfBuffer);
}

#[cfg(test)]
use serde::Deserialize;

#[cfg(test)]
#[derive(Deserialize, PartialEq, Debug)]
struct TestStruct {
    a: Vec<i8>,
    b: String,
    c: (u32, u32),
}

#[test]
fn test_struct() {
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

#[test]
fn test_f32() {
    let buf: &[u8] = &[0, 0, 0, 0];
    from_bytes_fail::<f32>(buf, Error::NotSupported);
}

#[test]
fn test_f64() {
    let buf: &[u8] = &[0, 0, 0, 0, 0, 0, 0, 0];
    from_bytes_fail::<f64>(buf, Error::NotSupported);
}

#[test]
fn test_char() {
    let buf: &[u8] = &[0, 0, 0, 0];
    from_bytes_fail::<char>(buf, Error::NotSupported);
}

#[cfg(test)]
use std::collections::HashMap;

#[test]
fn test_hash_map() {
    let buf: &[u8] = &[0, 0, 0, 0];
    from_bytes_fail::<HashMap<u8, u8>>(buf, Error::NotSupported);
}

#[cfg(test)]
#[derive(Deserialize, std::fmt::Debug, std::cmp::PartialEq)]
enum TestVariant {
    A(u8),
    B,
    C(String),
}

#[test]
fn test_variant() {
    let buf: &[u8] = &[0, 0, 0, 0];
    from_bytes_fail::<TestVariant>(buf, Error::NotSupported);
}

#[cfg(test)]
#[derive(Deserialize, std::fmt::Debug, std::cmp::PartialEq)]
enum TestEnum {
    A,
    B,
    C,
}

#[test]
fn test_enum() {
    let buf: &[u8] = &[0, 0, 0, 0];
    from_bytes_fail::<TestEnum>(buf, Error::NotSupported);
}

#[cfg(test)]
#[derive(Deserialize, std::fmt::Debug, std::cmp::PartialEq)]
struct UnitStruct;

#[test]
fn test_unit_struct() {
    let buf: &[u8] = &[0, 0, 0, 0];
    from_bytes_fail::<UnitStruct>(buf, Error::NotSupported);
}

#[test]
fn test_unit() {
    let buf: &[u8] = &[0, 0, 0, 0];
    from_bytes_fail::<()>(buf, Error::NotSupported);
}
