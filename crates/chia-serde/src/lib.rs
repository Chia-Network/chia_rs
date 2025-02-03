use std::fmt;

use serde::{
    de::{self, Visitor},
    Deserializer, Serializer,
};

pub fn ser_bytes<S>(value: &[u8], serializer: S, include_0x: bool) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if serializer.is_human_readable() {
        if include_0x {
            serializer.serialize_str(&format!("0x{}", hex::encode(value)))
        } else {
            serializer.serialize_str(&hex::encode(value))
        }
    } else {
        serializer.serialize_bytes(value)
    }
}

pub fn de_bytes<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: TryFrom<Vec<u8>>,
{
    if deserializer.is_human_readable() {
        struct HexOrBytesVisitor;

        impl Visitor<'_> for HexOrBytesVisitor {
            type Value = Vec<u8>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a byte buffer or hex string with an optional 0x prefix")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E> {
                Ok(v.to_vec())
            }

            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E> {
                Ok(v)
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if let Some(rest) = v.strip_prefix("0x") {
                    Ok(hex::decode(rest).map_err(de::Error::custom)?)
                } else if let Some(rest) = v.strip_prefix("0X") {
                    Ok(hex::decode(rest).map_err(de::Error::custom)?)
                } else {
                    Ok(hex::decode(v).map_err(de::Error::custom)?)
                }
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(&v)
            }
        }

        let bytes = deserializer.deserialize_any(HexOrBytesVisitor)?;
        let length = bytes.len();

        bytes.try_into().map_err(|_: T::Error| {
            de::Error::custom(format_args!(
                "Can't convert a byte buffer of length {length} to the output type."
            ))
        })
    } else {
        struct BytesVisitor;

        impl Visitor<'_> for BytesVisitor {
            type Value = Vec<u8>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a byte buffer")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E> {
                Ok(v.to_vec())
            }

            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E> {
                Ok(v)
            }
        }

        let bytes = deserializer.deserialize_byte_buf(BytesVisitor)?;
        let length = bytes.len();

        bytes.try_into().map_err(|_: T::Error| {
            de::Error::custom(format_args!(
                "Can't convert a byte buffer of length {length} to the output type."
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::Result;

    #[test]
    fn test_encode_hex() -> Result<()> {
        let original = b"hello";

        let mut ser = serde_json::Serializer::new(Vec::new());
        ser_bytes(original, &mut ser, false)?;
        let result = String::from_utf8(ser.into_inner())?;
        assert_eq!(result, "\"68656c6c6f\"");

        let mut de = serde_json::Deserializer::from_str(&result);
        let bytes = de_bytes::<_, Vec<u8>>(&mut de)?;
        assert_eq!(bytes, original);

        Ok(())
    }

    #[test]
    fn test_encode_0x() -> Result<()> {
        let original = b"hello";

        let mut ser = serde_json::Serializer::new(Vec::new());
        ser_bytes(original, &mut ser, true)?;
        let result = String::from_utf8(ser.into_inner())?;
        assert_eq!(result, "\"0x68656c6c6f\"");

        let mut de = serde_json::Deserializer::from_str(&result);
        let bytes = de_bytes::<_, Vec<u8>>(&mut de)?;
        assert_eq!(bytes, original);

        Ok(())
    }

    #[test]
    fn test_encode_binary() -> Result<()> {
        let original = b"hello";

        let mut output = Vec::new();
        let mut ser = bincode::Serializer::new(&mut output, bincode::options());
        ser_bytes(original, &mut ser, true)?;
        assert_eq!(hex::encode(&output), "0568656c6c6f");

        let mut de = bincode::Deserializer::from_slice(&output, bincode::options());
        let bytes = de_bytes::<_, Vec<u8>>(&mut de)?;
        assert_eq!(bytes, original);

        Ok(())
    }
}
