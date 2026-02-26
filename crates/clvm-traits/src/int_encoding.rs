const MAX_PADDING_BYTES: usize = 64;

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

/// # Panics
///
/// Panics if `LEN` is zero.
pub fn decode_number<const LEN: usize>(mut slice: &[u8], signed: bool) -> Option<[u8; LEN]> {
    assert!(LEN > 0, "Array length must be greater than zero");

    // Empty atoms are zero
    if slice.is_empty() {
        return Some([0; LEN]);
    }

    // Reject negative numbers for unsigned types
    if !signed && slice[0] & 0x80 != 0 {
        return None;
    }

    let was_negative = signed && slice[0] & 0x80 != 0;
    let padding_byte = if was_negative { 0xFF } else { 0x00 };
    let mut padding_bytes = 0;

    while slice.len() > LEN && slice[0] == padding_byte {
        if padding_bytes == MAX_PADDING_BYTES {
            return None;
        }

        slice = &slice[1..];
        padding_bytes += 1;
    }

    let is_negative = signed && slice[0] & 0x80 != 0;

    if slice.len() > LEN || (is_negative != was_negative) {
        return None;
    }

    let mut result = [padding_byte; LEN];
    let start = LEN - slice.len();

    result[start..].copy_from_slice(slice);

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    use clvmr::Allocator;
    use rstest::rstest;

    macro_rules! test_roundtrip {
        ( $num:expr, $signed:expr ) => {
            let mut allocator = Allocator::new();
            let ptr = allocator.new_number($num.into()).unwrap();
            let atom = allocator.atom(ptr);
            let expected = atom.as_ref();

            #[allow(unused_comparisons)]
            let encoded = encode_number(&$num.to_be_bytes(), $num < 0);
            assert_eq!(encoded, expected);

            let expected = $num.to_be_bytes();
            let decoded = decode_number(&encoded, $signed).unwrap();
            assert_eq!(decoded, expected);
        };
    }

    #[test]
    fn test_u8() {
        // We can test all combinations of u8 since there are only 256 possible values.
        for number in u8::MIN..=u8::MAX {
            test_roundtrip!(number, false);
        }
    }

    #[test]
    fn test_i8() {
        // We can test all combinations of i8 since there are only 256 possible values.
        for number in i8::MIN..=i8::MAX {
            test_roundtrip!(number, true);
        }
    }

    #[test]
    fn test_u16() {
        // We can test all combinations of u16 since there are only 65536 possible values.
        for number in u16::MIN..=u16::MAX {
            test_roundtrip!(number, false);
        }
    }

    #[test]
    fn test_i16() {
        // We can test all combinations of i16 since there are only 65536 possible values.
        for number in i16::MIN..=i16::MAX {
            test_roundtrip!(number, true);
        }
    }

    #[rstest]
    fn test_u32(
        #[values(
            0, 1, 10, 100, 1000, i8::MAX as u32, u8::MAX as u32, i16::MAX as u32, u16::MAX as u32, i32::MAX as u32, u32::MAX
        )]
        number: u32,
    ) {
        test_roundtrip!(number, false);
    }

    #[rstest]
    fn test_i32(
        #[values(
            0, 1, 10, 100, 1000, i8::MAX as i32, u8::MAX as i32, i16::MAX as i32, u16::MAX as i32, i32::MAX
        )]
        number: i32,
    ) {
        test_roundtrip!(number, true);
    }

    #[rstest]
    // Empty atoms are zero, regardless of sign
    #[case(&[], false, Some([0x00, 0x00]))]
    #[case(&[], true, Some([0x00, 0x00]))]
    // Single byte atoms are sign extended to fit the array
    #[case(&[0xFF], true, Some([0xFF, 0xFF]))]
    #[case(&[0xFE], true, Some([0xFF, 0xFE]))]
    #[case(&[0x80], true, Some([0xFF, 0x80]))]
    #[case(&[0x81], true, Some([0xFF, 0x81]))]
    #[case(&[0x7F], true, Some([0x00, 0x7F]))]
    #[case(&[0x01], true, Some([0x00, 0x01]))]
    #[case(&[0x00], true, Some([0x00, 0x00]))]
    // Leading zeros are padding bytes for positive numbers
    #[case(&[0x00], false, Some([0x00, 0x00]))]
    #[case(&[0x00], true, Some([0x00, 0x00]))]
    #[case(&[0x00, 0x00], false, Some([0x00, 0x00]))]
    #[case(&[0x00, 0x00], true, Some([0x00, 0x00]))]
    #[case(&[0x00, 0x00, 0x00], false, Some([0x00, 0x00]))]
    #[case(&[0x00, 0x00, 0x00], true, Some([0x00, 0x00]))]
    // You can have too many padding bytes
    #[case(&[0x00; MAX_PADDING_BYTES + 2], false, Some([0x00, 0x00]))]
    #[case(&[0x00; MAX_PADDING_BYTES + 2], true, Some([0x00, 0x00]))]
    #[case(&[0x00; MAX_PADDING_BYTES + 3], false, None)]
    #[case(&[0x00; MAX_PADDING_BYTES + 3], true, None)]
    #[case(&[0xFF; MAX_PADDING_BYTES + 2], true, Some([0xFF, 0xFF]))]
    #[case(&[0xFF; MAX_PADDING_BYTES + 3], true, None)]
    // If there's a non-padding byte that would result in exceeding the length, it's invalid
    #[case(&[0x01, 0x00, 0x00], false, None)]
    #[case(&[0x01, 0x00, 0x00], true, None)]
    // If the number after the padding bytes doesn't have the same sign as the original number, it's invalid
    #[case(&[0xFF, 0xFF, 0xFF, 0x00, 0x00], true, None)]
    #[case(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF], true, Some([0xFF, 0xFF]))]
    // Negative numbers aren't permitted for unsigned types
    #[case(&[0xFF], false, None)]
    #[case(&[0xFF, 0xFF], false, None)]
    // The padding byte makes this number positive
    #[case(&[0x00, 0xFF, 0xFF], false, Some([0xFF, 0xFF]))]
    fn test_decode(#[case] input: &[u8], #[case] signed: bool, #[case] expected: Option<[u8; 2]>) {
        assert_eq!(
            decode_number::<2>(input, signed),
            expected,
            "input: {input:?}, signed: {signed}, expected: {expected:?}",
        );
    }
}
