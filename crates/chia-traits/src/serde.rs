use std::{fmt, marker::PhantomData};

use serde::{
    de::{self, Visitor},
    Deserializer, Serializer,
};
use serde_with::{DeserializeAs, SerializeAs};

pub struct PreferPrefix;
pub struct AllowPrefix;
pub struct NoPrefix;
pub struct RequirePrefix;

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixKind {
    PreferPrefix,
    AllowPrefix,
    NoPrefix,
    RequirePrefix,
}

pub struct HexOrBytes<P = AllowPrefix>(PhantomData<P>);

pub fn serialize_hex_or_bytes<S>(
    source: &impl AsRef<[u8]>,
    serializer: S,
    kind: PrefixKind,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if serializer.is_human_readable() {
        let mut string = hex::encode(source);
        match kind {
            PrefixKind::PreferPrefix | PrefixKind::RequirePrefix => string.insert_str(0, "0x"),
            PrefixKind::AllowPrefix | PrefixKind::NoPrefix => {}
        }
        serializer.serialize_str(&string)
    } else {
        serializer.serialize_bytes(source.as_ref())
    }
}

pub fn deserialize_hex_or_bytes<'de, D, T>(deserializer: D, kind: PrefixKind) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: TryFrom<Vec<u8>>,
{
    if deserializer.is_human_readable() {
        struct HexOrBytesVisitor(PrefixKind);

        impl<'de> Visitor<'de> for HexOrBytesVisitor {
            type Value = Vec<u8>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self.0 {
                    PrefixKind::AllowPrefix | PrefixKind::PreferPrefix => formatter
                        .write_str("a byte buffer or hex string with an optional 0x prefix"),
                    PrefixKind::NoPrefix => formatter.write_str("a byte buffer or hex string"),
                    PrefixKind::RequirePrefix => {
                        formatter.write_str("a byte buffer or hex string with a 0x prefix")
                    }
                }
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
                if self.0 == PrefixKind::RequirePrefix
                    && !(v.starts_with("0x") || v.starts_with("0X"))
                {
                    return Err(de::Error::custom("Hex string missing 0x prefix"));
                }

                if matches!(
                    self.0,
                    PrefixKind::AllowPrefix | PrefixKind::PreferPrefix | PrefixKind::RequirePrefix
                ) {
                    if let Some(rest) = v.strip_prefix("0x") {
                        Ok(hex::decode(rest).map_err(de::Error::custom)?)
                    } else if let Some(rest) = v.strip_prefix("0X") {
                        Ok(hex::decode(rest).map_err(de::Error::custom)?)
                    } else {
                        Ok(hex::decode(v).map_err(de::Error::custom)?)
                    }
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

        let bytes = deserializer.deserialize_any(HexOrBytesVisitor(kind))?;
        let length = bytes.len();

        bytes.try_into().map_err(|_: T::Error| {
            de::Error::custom(format_args!(
                "Can't convert a byte buffer of length {length} to the output type."
            ))
        })
    } else {
        struct BytesVisitor;

        impl<'de> Visitor<'de> for BytesVisitor {
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

macro_rules! hex_or_bytes {
    ( $prefix:ident ) => {
        impl<T> SerializeAs<T> for HexOrBytes<$prefix>
        where
            T: AsRef<[u8]>,
        {
            fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serialize_hex_or_bytes(source, serializer, PrefixKind::$prefix)
            }
        }

        impl<'de, T> DeserializeAs<'de, T> for HexOrBytes<$prefix>
        where
            T: TryFrom<Vec<u8>>,
        {
            fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserialize_hex_or_bytes(deserializer, PrefixKind::$prefix)
            }
        }
    };
}

hex_or_bytes!(PreferPrefix);
hex_or_bytes!(AllowPrefix);
hex_or_bytes!(NoPrefix);
hex_or_bytes!(RequirePrefix);

#[cfg(test)]
mod tests {
    use de::DeserializeOwned;
    use fmt::Debug;
    use hex_literal::hex;
    use serde::{Deserialize, Serialize};
    use serde_with::serde_as;

    use super::*;

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct BytesDefault(#[serde_as(as = "HexOrBytes")] Vec<u8>);

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct BytesAllowPrefix(#[serde_as(as = "HexOrBytes<AllowPrefix>")] Vec<u8>);

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct BytesPreferPrefix(#[serde_as(as = "HexOrBytes<PreferPrefix>")] Vec<u8>);

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct BytesNoPrefix(#[serde_as(as = "HexOrBytes<NoPrefix>")] Vec<u8>);

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct BytesRequirePrefix(#[serde_as(as = "HexOrBytes<RequirePrefix>")] Vec<u8>);

    fn roundtrip_json<T>(value: &T, hex: &str)
    where
        T: Debug + PartialEq + Serialize + DeserializeOwned,
    {
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, format!("\"{hex}\""));

        let roundtrip: T = serde_json::from_str(&json).unwrap();
        assert_eq!(&roundtrip, value);
    }

    fn try_parse_json<T>(hex: &str, value: &T)
    where
        T: Debug + PartialEq + DeserializeOwned,
    {
        let actual = serde_json::from_str::<T>(&format!("\"{hex}\"")).unwrap();
        assert_eq!(&actual, value);
    }

    fn try_error_json<T>(hex: &str)
    where
        T: Debug + PartialEq + DeserializeOwned,
    {
        let actual = serde_json::from_str::<T>(&format!("\"{hex}\""));
        assert!(actual.is_err());
    }

    fn roundtrip_binary<T>(value: &T, hex: &str)
    where
        T: Debug + PartialEq + Serialize + DeserializeOwned,
    {
        let bytes = bincode::serialize(value).unwrap();
        assert_eq!(hex::encode(&bytes), hex);

        let roundtrip: T = bincode::deserialize(&bytes).unwrap();
        assert_eq!(&roundtrip, value);
    }

    #[test]
    fn test_bytes_as_json() {
        roundtrip_json(&BytesDefault(hex!("cafef00d").to_vec()), "cafef00d");
        try_parse_json("0xcafef00d", &BytesDefault(hex!("cafef00d").to_vec()));
    }

    #[test]
    fn test_bytes_allow_prefix_as_json() {
        roundtrip_json(&BytesAllowPrefix(hex!("cafef00d").to_vec()), "cafef00d");
        try_parse_json("0xcafef00d", &BytesAllowPrefix(hex!("cafef00d").to_vec()));
    }

    #[test]
    fn test_bytes_prefer_prefix_as_json() {
        roundtrip_json(&BytesPreferPrefix(hex!("cafef00d").to_vec()), "0xcafef00d");
        try_parse_json("cafef00d", &BytesPreferPrefix(hex!("cafef00d").to_vec()));
    }

    #[test]
    fn test_bytes_no_prefix_as_json() {
        roundtrip_json(&BytesNoPrefix(hex!("cafef00d").to_vec()), "cafef00d");
        try_error_json::<BytesNoPrefix>("0xcafef00d");
    }

    #[test]
    fn test_bytes_require_prefix_as_json() {
        roundtrip_json(&BytesRequirePrefix(hex!("cafef00d").to_vec()), "0xcafef00d");
        try_error_json::<BytesRequirePrefix>("cafef00d");
    }

    #[test]
    fn test_bytes_as_binary() {
        roundtrip_binary(
            &BytesDefault(hex!("cafef00d").to_vec()),
            "0400000000000000cafef00d",
        );
    }

    #[test]
    fn test_bytes_allow_prefix_as_binary() {
        roundtrip_binary(
            &BytesAllowPrefix(hex!("cafef00d").to_vec()),
            "0400000000000000cafef00d",
        );
    }

    #[test]
    fn test_bytes_prefer_prefix_as_binary() {
        roundtrip_binary(
            &BytesPreferPrefix(hex!("cafef00d").to_vec()),
            "0400000000000000cafef00d",
        );
    }

    #[test]
    fn test_bytes_no_prefix_as_binary() {
        roundtrip_binary(
            &BytesNoPrefix(hex!("cafef00d").to_vec()),
            "0400000000000000cafef00d",
        );
    }

    #[test]
    fn test_bytes_require_prefix_as_binary() {
        roundtrip_binary(
            &BytesRequirePrefix(hex!("cafef00d").to_vec()),
            "0400000000000000cafef00d",
        );
    }
}
