use super::sanitize_int::sanitize_uint;
use super::validation_error::{atom, ErrorCode, ValidationErr};
use clvmr::allocator::{Allocator, NodePtr};
use clvmr::op_utils::u64_from_bytes;

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
    match sanitize_uint(a, n, 8, code) {
        Err(ValidationErr(n, ErrorCode::NegativeAmount)) => Err(ValidationErr(n, code)),
        Err(ValidationErr(n, ErrorCode::AmountExceedsMaximum)) => Err(ValidationErr(n, code)),
        Err(r) => Err(r),
        Ok(r) => Ok(u64_from_bytes(r)),
    }
}

pub fn parse_create_coin_amount(a: &Allocator, n: NodePtr) -> Result<u64, ValidationErr> {
    // amounts are not allowed to exceed 2^64. i.e. 8 bytes
    let buf = sanitize_uint(a, n, 8, ErrorCode::InvalidCoinAmount)?;
    Ok(u64_from_bytes(buf))
}

// a negative height is always true. In this case the
// condition can be ignored and this functon returns 0
pub fn parse_height(a: &Allocator, n: NodePtr, code: ErrorCode) -> Result<u32, ValidationErr> {
    // heights are not allowed to exceed 2^32. i.e. 4 bytes
    match sanitize_uint(a, n, 4, code) {
        // Height is always positive, so a negative requirement is always true,
        // just like 0.
        Err(ValidationErr(_, ErrorCode::NegativeAmount)) => Ok(0),
        Err(ValidationErr(n, ErrorCode::AmountExceedsMaximum)) => Err(ValidationErr(n, code)),
        Err(r) => Err(r),
        Ok(r) => Ok(u64_from_bytes(r) as u32),
    }
}

// negative seconds are always valid conditions, and will return 0
pub fn parse_seconds(a: &Allocator, n: NodePtr, code: ErrorCode) -> Result<u64, ValidationErr> {
    // seconds are not allowed to exceed 2^64. i.e. 8 bytes
    match sanitize_uint(a, n, 8, code) {
        // seconds is always positive, so a negative requirement is always true,
        // we don't need to include this condition
        Err(ValidationErr(_, ErrorCode::NegativeAmount)) => Ok(0),
        Err(ValidationErr(n, ErrorCode::AmountExceedsMaximum)) => Err(ValidationErr(n, code)),
        Err(r) => Err(r),
        Ok(r) => Ok(u64_from_bytes(r)),
    }
}

// a negative height is a failure, but exceeding the max is a no-op.
// This is used for parsing assert_before_height conditions
pub fn parse_positive_height(
    a: &Allocator,
    n: NodePtr,
    code: ErrorCode,
) -> Result<Option<u32>, ValidationErr> {
    // heights are not allowed to exceed 2^32. i.e. 4 bytes
    match sanitize_uint(a, n, 4, code) {
        Err(ValidationErr(n, ErrorCode::NegativeAmount)) => Err(ValidationErr(n, code)),
        Err(ValidationErr(_, ErrorCode::AmountExceedsMaximum)) => Ok(None),
        Err(r) => Err(r),
        Ok(r) => Ok(Some(u64_from_bytes(r) as u32)),
    }
}

// negative seconds are a failure, exceeding the max limit is no-op.
// this is used for parsing assert_before_seconds conditions.
pub fn parse_positive_seconds(
    a: &Allocator,
    n: NodePtr,
    code: ErrorCode,
) -> Result<Option<u64>, ValidationErr> {
    // seconds are not allowed to exceed 2^64. i.e. 8 bytes
    match sanitize_uint(a, n, 8, code) {
        Err(ValidationErr(n, ErrorCode::NegativeAmount)) => Err(ValidationErr(n, code)),
        Err(ValidationErr(_, ErrorCode::AmountExceedsMaximum)) => Ok(None),
        Err(r) => Err(r),
        Ok(r) => Ok(Some(u64_from_bytes(r))),
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
use rstest::rstest;

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

#[cfg(test)]
fn create_amount_tester(buf: &[u8]) -> Result<u64, ValidationErr> {
    let mut a = Allocator::new();
    let n = a.new_atom(buf).unwrap();

    parse_create_coin_amount(&mut a, n)
}

#[test]
fn test_sanitize_create_coin_amount() {
    // negative coin amounts are not allowed
    assert_eq!(
        create_amount_tester(&[0xff]).unwrap_err().1,
        ErrorCode::NegativeAmount
    );
    // amounts aren't allowed to be too big
    let large_buf = [0x7f, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(
        create_amount_tester(&large_buf).unwrap_err().1,
        ErrorCode::AmountExceedsMaximum
    );

    // this is small enough though
    assert_eq!(
        create_amount_tester(&[0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]),
        Ok(0xffffffffffffffff)
    );
}

#[cfg(test)]
fn height_tester(buf: &[u8]) -> Result<Option<u64>, ValidationErr> {
    let mut a = Allocator::new();
    let n = a.new_atom(buf).unwrap();
    Ok(Some(
        parse_height(&mut a, n, ErrorCode::AssertHeightAbsolute)? as u64,
    ))
}

#[cfg(test)]
fn seconds_tester(buf: &[u8]) -> Result<Option<u64>, ValidationErr> {
    let mut a = Allocator::new();
    let n = a.new_atom(buf).unwrap();
    Ok(Some(parse_seconds(
        &mut a,
        n,
        ErrorCode::AssertSecondsAbsolute,
    )?))
}

#[cfg(test)]
fn positive_height_tester(buf: &[u8]) -> Result<Option<u64>, ValidationErr> {
    let mut a = Allocator::new();
    let n = a.new_atom(buf).unwrap();
    Ok(parse_positive_height(&mut a, n, ErrorCode::AssertBeforeHeightAbsolute)?.map(|v| v as u64))
}

#[cfg(test)]
fn positive_seconds_tester(buf: &[u8]) -> Result<Option<u64>, ValidationErr> {
    let mut a = Allocator::new();
    let n = a.new_atom(buf).unwrap();
    parse_positive_seconds(&mut a, n, ErrorCode::AssertBeforeSecondsAbsolute)
}

#[cfg(test)]
#[rstest]
// == parse_height
#[case(height_tester, &[0x80], Some(0))]
// negative values are no-ops
#[case(height_tester, &[0xff], Some(0))]
#[case(height_tester, &[0xff, 0], Some(0))]
// leading zeros are sometimes necessary to make values positive
#[case(height_tester, &[0, 0xff], Some(0xff))]
// this is small enough
#[case(height_tester, &[0, 0xff, 0xff, 0xff, 0xff], Some(0xffffffff))]
// == parse_seconds
#[case(seconds_tester, &[0x80], Some(0))]
// negative values are no-ops
#[case(seconds_tester, &[0xff], Some(0))]
#[case(seconds_tester, &[0xff, 0], Some(0))]
// leading zeros are sometimes necessary to make values positive
#[case(seconds_tester, &[0, 0xff], Some(0xff))]
// this is small enough
#[case(seconds_tester, &[0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff], Some(0xffffffffffffffff))]
// == parse_positive_height
// leading zeros are sometimes necessary to make values positive
#[case(positive_height_tester, &[0, 0xff], Some(0xff))]
// this is small enough
#[case(positive_height_tester, &[0, 0xff, 0xff, 0xff, 0xff], Some(0xffffffff))]
// positive heights are allowed to be > 2^32 (i.e. 5 bytes). it's a no-op
#[case(positive_height_tester, &[0x01, 0xff, 0xff, 0xff, 0xff], None)]
// == parse_positive_seconds
// leading zeros are sometimes necessary to make values positive
#[case(positive_seconds_tester, &[0, 0xff], Some(0xff))]
// this is small enough
#[case(positive_seconds_tester, &[0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff], Some(0xffffffffffffffff))]
// before-seconds are allowed to be > 2^64 (i.e. 9 bytes)
#[case(positive_seconds_tester, &[0x01, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff], None)]
fn test_parse_ok(
    #[case] fun: impl Fn(&[u8]) -> Result<Option<u64>, ValidationErr>,
    #[case] buf: &[u8],
    #[case] expected: Option<u64>,
) {
    println!("test case: {:?} expect: {:?}", buf, expected);
    // negative heights can be ignored
    assert_eq!(fun(buf).unwrap(), expected);
}

#[cfg(test)]
#[rstest]
// == parse_height
#[case(height_tester, &[0, 0, 0, 0xff], ErrorCode::AssertHeightAbsolute)]
#[case(height_tester, &[0, 0, 0, 0x80], ErrorCode::AssertHeightAbsolute)]
#[case(height_tester, &[0, 0, 0, 0x7f], ErrorCode::AssertHeightAbsolute)]
#[case(height_tester, &[0, 0, 0], ErrorCode::AssertHeightAbsolute)]
#[case(height_tester, &[0], ErrorCode::AssertHeightAbsolute)]
// heights aren't allowed to be > 2^32 (i.e. 5 bytes)
#[case(height_tester, &[0x01, 0xff, 0xff, 0xff, 0xff], ErrorCode::AssertHeightAbsolute)]
// == parse_seconds
#[case(seconds_tester, &[0, 0, 0, 0xff], ErrorCode::AssertSecondsAbsolute)]
#[case(seconds_tester, &[0, 0, 0, 0x80], ErrorCode::AssertSecondsAbsolute)]
#[case(seconds_tester, &[0, 0, 0, 0x7f], ErrorCode::AssertSecondsAbsolute)]
#[case(seconds_tester, &[0, 0, 0], ErrorCode::AssertSecondsAbsolute)]
#[case(seconds_tester, &[0], ErrorCode::AssertSecondsAbsolute)]
// positive seconds are allowed to be > 2^64 (i.e. 9 bytes)
#[case(seconds_tester, &[0x01, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff], ErrorCode::AssertSecondsAbsolute)]
// == parse_positive_height
#[case(positive_height_tester, &[0, 0, 0, 0xff], ErrorCode::AssertBeforeHeightAbsolute)]
#[case(positive_height_tester, &[0, 0, 0, 0x80], ErrorCode::AssertBeforeHeightAbsolute)]
#[case(positive_height_tester, &[0, 0, 0, 0x7f], ErrorCode::AssertBeforeHeightAbsolute)]
#[case(positive_height_tester, &[0, 0, 0], ErrorCode::AssertBeforeHeightAbsolute)]
#[case(positive_height_tester, &[0], ErrorCode::AssertBeforeHeightAbsolute)]
// negative values are failures
#[case(positive_height_tester, &[0x80], ErrorCode::AssertBeforeHeightAbsolute)]
#[case(positive_height_tester, &[0xff], ErrorCode::AssertBeforeHeightAbsolute)]
#[case(positive_height_tester, &[0xff, 0], ErrorCode::AssertBeforeHeightAbsolute)]
// == parse_positive_seconds
#[case(positive_seconds_tester, &[0, 0, 0, 0xff], ErrorCode::AssertBeforeSecondsAbsolute)]
#[case(positive_seconds_tester, &[0, 0, 0, 0x80], ErrorCode::AssertBeforeSecondsAbsolute)]
#[case(positive_seconds_tester, &[0, 0, 0, 0x7f], ErrorCode::AssertBeforeSecondsAbsolute)]
#[case(positive_seconds_tester, &[0, 0, 0], ErrorCode::AssertBeforeSecondsAbsolute)]
#[case(positive_seconds_tester, &[0], ErrorCode::AssertBeforeSecondsAbsolute)]
// negative values are failures
#[case(positive_seconds_tester, &[0x80], ErrorCode::AssertBeforeSecondsAbsolute)]
#[case(positive_seconds_tester, &[0xff], ErrorCode::AssertBeforeSecondsAbsolute)]
#[case(positive_seconds_tester, &[0xff, 0], ErrorCode::AssertBeforeSecondsAbsolute)]
fn test_parse_fail(
    #[case] fun: impl Fn(&[u8]) -> Result<Option<u64>, ValidationErr>,
    #[case] buf: &[u8],
    #[case] expected: ErrorCode,
) {
    println!("test case: {:?} expect: {:?}", buf, expected);
    // negative heights can be ignored
    assert_eq!(fun(buf).unwrap_err().1, expected);
}

#[test]
fn test_parse_height_pair() {
    let mut a = Allocator::new();
    let pair = a.new_pair(a.null(), a.null()).unwrap();
    assert_eq!(
        parse_height(&mut a, pair, ErrorCode::AssertHeightAbsolute),
        Err(ValidationErr(pair, ErrorCode::AssertHeightAbsolute))
    );
}

#[test]
fn test_parse_seconds_pair() {
    let mut a = Allocator::new();
    let pair = a.new_pair(a.null(), a.null()).unwrap();
    assert_eq!(
        parse_seconds(&mut a, pair, ErrorCode::AssertSecondsAbsolute),
        Err(ValidationErr(pair, ErrorCode::AssertSecondsAbsolute))
    );
}

#[test]
fn test_parse_positive_height_pair() {
    let mut a = Allocator::new();
    let pair = a.new_pair(a.null(), a.null()).unwrap();
    assert_eq!(
        parse_positive_height(&mut a, pair, ErrorCode::AssertHeightAbsolute),
        Err(ValidationErr(pair, ErrorCode::AssertHeightAbsolute))
    );
}

#[test]
fn test_parse_positive_seconds_pair() {
    let mut a = Allocator::new();
    let pair = a.new_pair(a.null(), a.null()).unwrap();
    assert_eq!(
        parse_positive_seconds(&mut a, pair, ErrorCode::AssertSecondsAbsolute),
        Err(ValidationErr(pair, ErrorCode::AssertSecondsAbsolute))
    );
}
