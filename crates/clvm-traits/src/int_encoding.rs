pub fn encode_number(slice: &[u8], negative: bool) -> Vec<u8> {
    let mut start = 0;
    let pad_byte = if negative { 0xFF } else { 0x00 };

    // Skip leading pad bytes
    while start < slice.len() && slice[start] == pad_byte {
        start += 1;
    }

    let needs_padding = if negative {
        start == slice.len() || (slice[start] & 0x80) == 0
    } else {
        start < slice.len() && (slice[start] & 0x80) != 0
    };

    let mut result = Vec::with_capacity(if needs_padding {
        slice.len() - start + 1
    } else {
        slice.len() - start
    });

    if needs_padding {
        result.push(pad_byte);
    }

    result.extend_from_slice(&slice[start..]);
    result
}

pub fn decode_number<const LEN: usize>(mut slice: &[u8], signed: bool) -> Option<[u8; LEN]> {
    let negative = signed && !slice.is_empty() && slice[0] & 0x80 != 0;
    let padding_byte = if negative { 0xFF } else { 0x00 };

    if slice.len() > LEN && slice[0] == padding_byte {
        slice = &slice[slice.len() - LEN..];
    }

    if slice.len() > LEN {
        return None;
    }

    assert!(slice.len() <= LEN);

    let mut result = [padding_byte; LEN];
    let start = LEN - slice.len();

    result[start..].copy_from_slice(slice);

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    use clvmr::Allocator;

    macro_rules! test_roundtrip {
        ( $num:expr, $signed:expr ) => {
            let mut allocator = Allocator::new();
            let ptr = allocator.new_number($num.into()).unwrap();
            let atom = allocator.atom(ptr);
            let expected = atom.as_ref();

            #[allow(unused_comparisons)]
            let encoded = encode_number(&$num.to_be_bytes(), $num < 0);
            assert_eq!(expected, encoded);

            let expected = $num.to_be_bytes();
            let decoded = decode_number(&encoded, $signed).unwrap();
            assert_eq!(expected, decoded);
        };
    }

    #[test]
    fn test_signed_encoding() {
        test_roundtrip!(0i32, true);
        test_roundtrip!(1i32, true);
        test_roundtrip!(2i32, true);
        test_roundtrip!(3i32, true);
        test_roundtrip!(255i32, true);
        test_roundtrip!(4716i32, true);
        test_roundtrip!(-255i32, true);
        test_roundtrip!(-10i32, true);
        test_roundtrip!(i32::MIN, true);
        test_roundtrip!(i32::MAX, true);
    }

    #[test]
    fn test_unsigned_encoding() {
        test_roundtrip!(0u32, false);
        test_roundtrip!(1u32, false);
        test_roundtrip!(2u32, false);
        test_roundtrip!(3u32, false);
        test_roundtrip!(255u32, false);
        test_roundtrip!(u32::MAX, false);
    }
}
