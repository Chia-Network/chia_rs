use super::sanitize_int::{sanitize_uint, SanitizedUint};
use super::validation_error::{atom, ErrorCode, ValidationErr};
use clvmr::allocator::{Allocator, NodePtr};

pub fn sanitize_hash(
    a: &Allocator,
    n: NodePtr,
    size: usize,
    code: ErrorCode,
) -> Result<NodePtr, ValidationErr> {
    let buf = atom(a, n, code)?;

    if buf.len() != size {
        Err(ValidationErr(n, code))
    } else {
        Ok(n)
    }
}

pub fn parse_amount(a: &Allocator, n: NodePtr, code: ErrorCode) -> Result<u64, ValidationErr> {
    // amounts are not allowed to exceed 2^64. i.e. 8 bytes
    match sanitize_uint(a, n, 8, code)? {
        SanitizedUint::NegativeOverflow => Err(ValidationErr(n, code)),
        SanitizedUint::PositiveOverflow => Err(ValidationErr(n, code)),
        SanitizedUint::Ok(r) => Ok(r),
    }
}

pub fn sanitize_announce_msg(
    a: &Allocator,
    n: NodePtr,
    code: ErrorCode,
) -> Result<NodePtr, ValidationErr> {
    let buf = atom(a, n, code)?;

    if buf.len() > 1024 {
        Err(ValidationErr(n, code))
    } else {
        Ok(n)
    }
}

#[cfg(test)]
fn zero_vec(len: usize) -> Vec<u8> {
    let mut ret = Vec::<u8>::new();
    for _i in 0..len {
        ret.push(0);
    }
    ret
}

#[test]
fn test_sanitize_hash() {
    let mut a = Allocator::new();
    let short = zero_vec(31);
    let valid = zero_vec(32);
    let long = zero_vec(33);

    let short_n = a.new_atom(&short).unwrap();
    assert_eq!(
        sanitize_hash(&a, short_n, 32, ErrorCode::InvalidCondition),
        Err(ValidationErr(short_n, ErrorCode::InvalidCondition))
    );
    let valid_n = a.new_atom(&valid).unwrap();
    assert_eq!(
        sanitize_hash(&a, valid_n, 32, ErrorCode::InvalidCondition),
        Ok(valid_n)
    );
    let long_n = a.new_atom(&long).unwrap();
    assert_eq!(
        sanitize_hash(&a, long_n, 32, ErrorCode::InvalidCondition),
        Err(ValidationErr(long_n, ErrorCode::InvalidCondition))
    );

    let pair = a.new_pair(short_n, long_n).unwrap();
    assert_eq!(
        sanitize_hash(&a, pair, 32, ErrorCode::InvalidCondition),
        Err(ValidationErr(pair, ErrorCode::InvalidCondition))
    );
}

#[test]
fn test_sanitize_announce_msg() {
    let mut a = Allocator::new();
    let valid = zero_vec(1024);
    let valid_n = a.new_atom(&valid).unwrap();
    assert_eq!(
        sanitize_announce_msg(&a, valid_n, ErrorCode::InvalidCondition),
        Ok(valid_n)
    );

    let long = zero_vec(1025);
    let long_n = a.new_atom(&long).unwrap();
    assert_eq!(
        sanitize_announce_msg(&a, long_n, ErrorCode::InvalidCondition),
        Err(ValidationErr(long_n, ErrorCode::InvalidCondition))
    );

    let pair = a.new_pair(valid_n, long_n).unwrap();
    assert_eq!(
        sanitize_announce_msg(&a, pair, ErrorCode::InvalidCondition),
        Err(ValidationErr(pair, ErrorCode::InvalidCondition))
    );
}

#[cfg(test)]
fn amount_tester(buf: &[u8]) -> Result<u64, ValidationErr> {
    let mut a = Allocator::new();
    let n = a.new_atom(buf).unwrap();

    parse_amount(&mut a, n, ErrorCode::InvalidCoinAmount)
}

#[test]
fn test_sanitize_amount() {
    // negative amounts are not allowed
    assert_eq!(
        amount_tester(&[0x80]).unwrap_err().1,
        ErrorCode::InvalidCoinAmount
    );
    assert_eq!(
        amount_tester(&[0xff]).unwrap_err().1,
        ErrorCode::InvalidCoinAmount
    );
    assert_eq!(
        amount_tester(&[0xff, 0]).unwrap_err().1,
        ErrorCode::InvalidCoinAmount
    );

    // leading zeros are somtimes necessary to make values positive
    assert_eq!(amount_tester(&[0, 0xff]), Ok(0xff));
    // but are disallowed when they are redundant
    assert_eq!(
        amount_tester(&[0, 0, 0, 0xff]).unwrap_err().1,
        ErrorCode::InvalidCoinAmount
    );
    assert_eq!(
        amount_tester(&[0, 0, 0, 0x80]).unwrap_err().1,
        ErrorCode::InvalidCoinAmount
    );
    assert_eq!(
        amount_tester(&[0, 0, 0, 0x7f]).unwrap_err().1,
        ErrorCode::InvalidCoinAmount
    );
    assert_eq!(
        amount_tester(&[0, 0, 0]).unwrap_err().1,
        ErrorCode::InvalidCoinAmount
    );

    // amounts aren't allowed to be too big
    assert_eq!(
        amount_tester(&[0x7f, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
            .unwrap_err()
            .1,
        ErrorCode::InvalidCoinAmount
    );

    // this is small enough though
    assert_eq!(
        amount_tester(&[0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]),
        Ok(0xffffffffffffffff)
    );
}
