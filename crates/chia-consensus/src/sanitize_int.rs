use super::validation_error::{atom, ValidationErr};
use clvmr::allocator::{Allocator, NodePtr};

use clvmr::op_utils::u64_from_bytes;

#[derive(PartialEq, Debug)]
pub enum SanitizedUint {
    Ok(u64),
    PositiveOverflow,
    NegativeOverflow,
}

pub fn sanitize_uint(
    a: &Allocator,
    n: NodePtr,
    max_size: usize,
    make_err: fn(NodePtr) -> ValidationErr,
) -> Result<SanitizedUint, ValidationErr> {
    assert!(max_size <= 8);

    let buf = atom(a, n, make_err)?;
    let buf = buf.as_ref();

    if buf.is_empty() {
        return Ok(SanitizedUint::Ok(0));
    }

    if (buf[0] & 0x80) != 0 {
        return Ok(SanitizedUint::NegativeOverflow);
    }

    if buf == [0_u8] || (buf.len() > 1 && buf[0] == 0 && (buf[1] & 0x80) == 0) {
        return Err(make_err(n)); // changed
    }

    let size_limit = if buf[0] == 0 { max_size + 1 } else { max_size };

    if buf.len() > size_limit {
        return Ok(SanitizedUint::PositiveOverflow);
    }

    Ok(SanitizedUint::Ok(u64_from_bytes(buf)))
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

    let e = ValidationErr::InvalidCoinAmount;
    let no_leading_zero = a.new_substr(atom, 0, 8).unwrap();
    // this is a negative number, not allowed
    assert!(sanitize_uint(&a, no_leading_zero, 8, e) == Ok(SanitizedUint::NegativeOverflow));

    let just_zeros = a.new_substr(atom, 10, 70).unwrap();
    // a zero value must be represented by an empty atom
    assert!(matches!(
        sanitize_uint(&a, just_zeros, 8, e).unwrap_err(),
        ValidationErr::InvalidCoinAmount
    ));

    let a1 = a.new_substr(atom, 1, 101).unwrap();
    assert!(matches!(
        sanitize_uint(&a, a1, 8, e).unwrap_err(),
        ValidationErr::InvalidCoinAmount
    ));

    let a1 = a.new_substr(atom, 1, 101).unwrap();
    assert!(matches!(
        sanitize_uint(&a, a1, 8, e).unwrap_err(),
        ValidationErr::InvalidCoinAmount
    ));

    // a new all-zeros range
    let a1 = a.new_substr(atom, 1000, 1024).unwrap();
    assert!(matches!(
        sanitize_uint(&a, a1, 8, e).unwrap_err(),
        ValidationErr::InvalidCoinAmount
    ));

    let exceed_maximum = a.new_substr(atom, 100, 110).unwrap();
    assert_eq!(
        sanitize_uint(&a, exceed_maximum, 8, e),
        Ok(SanitizedUint::PositiveOverflow)
    );
}
