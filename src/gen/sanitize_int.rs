use super::validation_error::{atom, ErrorCode, ValidationErr};
use clvmr::allocator::{Allocator, NodePtr};

pub fn sanitize_uint(
    a: &Allocator,
    n: NodePtr,
    max_size: usize,
    code: ErrorCode,
) -> Result<&[u8], ValidationErr> {
    assert!(max_size <= 8);

    let buf = atom(a, n, code)?;

    if buf.is_empty() {
        return Ok(&[]);
    }

    // if the top bit is set, it's a negative number
    if (buf[0] & 0x80) != 0 {
        return Err(ValidationErr(n, ErrorCode::NegativeAmount));
    }

    // we only allow a leading zero if it's used to prevent a value to otherwise
    // be interpreted as a negative integer. i.e. if the next top bit is set
    // all other leading zeros are invalid
    if buf == [0_u8] || (buf.len() > 1 && buf[0] == 0 && (buf[1] & 0x80) == 0) {
        return Err(ValidationErr(n, code));
    }

    // strip the leading zero byte if there is one
    let size_limit = if buf[0] == 0 { max_size + 1 } else { max_size };

    // if there are too many bytes left in the value, it's too big
    if buf.len() > size_limit {
        return Err(ValidationErr(n, ErrorCode::AmountExceedsMaximum));
    }

    Ok(buf)
}

#[test]
fn test_sanitize_uint() {
    let mut a = Allocator::new();

    // start with one big buffer.
    let atom = {
        let mut buf = Vec::<u8>::new();
        for _i in 0..1024 {
            buf.push(0);
        }

        // make some of the bytes non-zero
        buf[0] = 0xff;
        buf[100] = 0x7f;
        buf[1023] = 0xff;
        a.new_atom(&buf)
    }
    .unwrap();

    let e = ErrorCode::InvalidCoinAmount;
    let no_leading_zero = a.new_substr(atom, 0, 8).unwrap();
    // this is a negative number, not allowed
    assert!(sanitize_uint(&a, no_leading_zero, 8, e).is_err());

    let just_zeros = a.new_substr(atom, 10, 70).unwrap();
    // a zero value must be represented by an empty atom
    assert_eq!(
        sanitize_uint(&a, just_zeros, 8, e).unwrap_err().1,
        ErrorCode::InvalidCoinAmount
    );

    let a1 = a.new_substr(atom, 1, 101).unwrap();
    assert_eq!(
        sanitize_uint(&a, a1, 8, e).unwrap_err().1,
        ErrorCode::InvalidCoinAmount
    );

    let a1 = a.new_substr(atom, 1, 101).unwrap();
    assert_eq!(
        sanitize_uint(&a, a1, 8, e).unwrap_err().1,
        ErrorCode::InvalidCoinAmount
    );

    // a new all-zeros range
    let a1 = a.new_substr(atom, 1000, 1024).unwrap();
    assert_eq!(
        sanitize_uint(&a, a1, 8, e).unwrap_err().1,
        ErrorCode::InvalidCoinAmount
    );

    let exceed_maximum = a.new_substr(atom, 100, 110).unwrap();
    assert_eq!(
        sanitize_uint(&a, exceed_maximum, 8, e).unwrap_err().1,
        ErrorCode::AmountExceedsMaximum
    );
}
