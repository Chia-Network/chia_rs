use super::sanitize_int::{sanitize_uint, SanitizedUint};
use super::validation_error::{atom, ValidationErr};
use clvmr::allocator::{Allocator, NodePtr};

pub fn sanitize_hash(
    a: &Allocator,
    n: NodePtr,
    size: usize,
    make_err: fn(NodePtr) -> ValidationErr,
) -> Result<NodePtr, ValidationErr> {
    let buf = atom(a, n, make_err)?; // assumes `atom` now also uses this pattern

    if buf.as_ref().len() == size {
        Ok(n)
    } else {
        Err(make_err(n))
    }
}

pub fn parse_amount(
    a: &Allocator,
    n: NodePtr,
    make_err: fn(NodePtr) -> ValidationErr,
) -> Result<u64, ValidationErr> {
    match sanitize_uint(a, n, 8, make_err)? {
        SanitizedUint::NegativeOverflow | SanitizedUint::PositiveOverflow => {
            Err(make_err(n))
        }
        SanitizedUint::Ok(r) => Ok(r),
    }
}

pub fn sanitize_announce_msg(
    a: &Allocator,
    n: NodePtr,
    code: fn(NodePtr) -> ValidationErr,
) -> Result<NodePtr, ValidationErr> {
    let buf = atom(a, n, code)?;

    if buf.as_ref().len() > 1024 {
        Err(make_err(n))
    } else {
        Ok(n)
    }
}

pub fn sanitize_message_mode(a: &Allocator, node: NodePtr) -> Result<u32, ValidationErr> {
    let Some(mode) = a.small_number(node) else {
        return Err(ValidationErr::InvalidMessageMode(node));
    };
    // only 6 bits are allowed to be set
    if (mode & !0b11_1111) != 0 {
        return Err(ValidationErr::InvalidMessageMode(node));
    }
    Ok(mode)
}

#[cfg(test)]
use rstest::rstest;

#[cfg(test)]
#[rstest]
#[case(0, true)]
#[case(-1, false)]
#[case(1, true)]
#[case(10_000_000_000, false)]
#[case(0xffff_ffff_ffff, false)]
#[case(-0xffff_ffff_ffff, false)]
#[case(0b100_1001, false)]
#[case(0b00_1001, true)]
#[case(0b01_0010, true)]
#[case(0b10_0100, true)]
#[case(0b10_1101, true)]
#[case(0b10_0001, true)]
#[case(0b11_1111, true)]
#[case(0b11_1100, true)]
#[case(0b10_0111, true)]
#[case(0b00_0111, true)]
#[case(0b11_1000, true)]
fn test_sanitize_mode(#[case] value: i64, #[case] pass: bool) {
    let mut a = Allocator::new();
    let node = a.new_number(value.into()).unwrap();

    let ret = sanitize_message_mode(&a, node);
    if pass {
        assert_eq!(i64::from(ret.unwrap()), value);
    } else {
        assert!(matches!(ret.unwrap_err().1, ValidationErr::InvalidMessageMode));
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
        sanitize_hash(&a, short_n, 32, ValidationErr::InvalidCondition),
        Err(ValidationErr::InvalidCondition(short_n))
    );
    let valid_n = a.new_atom(&valid).unwrap();
    assert_eq!(
        sanitize_hash(&a, valid_n, 32, ValidationErr::InvalidCondition),
        Ok(valid_n)
    );
    let long_n = a.new_atom(&long).unwrap();
    assert_eq!(
        sanitize_hash(&a, long_n, 32, ValidationErr::InvalidCondition),
        Err(ValidationErr::InvalidCondition(long_n))
    );

    let pair = a.new_pair(short_n, long_n).unwrap();
    assert_eq!(
        sanitize_hash(&a, pair, 32, ValidationCode::InvalidCondition),
        Err(ValidationErr::InvalidCondition(pair))
    );
}

#[test]
fn test_sanitize_announce_msg() {
    let mut a = Allocator::new();
    let valid = zero_vec(1024);
    let valid_n = a.new_atom(&valid).unwrap();
    assert_eq!(
        sanitize_announce_msg(&a, valid_n, ValidationErr::InvalidCondition),
        Ok(valid_n)
    );

    let long = zero_vec(1025);
    let long_n = a.new_atom(&long).unwrap();
    assert_eq!(
        sanitize_announce_msg(&a, long_n, ValidationErr::InvalidCondition),
        Err(ValidationErr::InvalidCondition(long_n))
    );

    let pair = a.new_pair(valid_n, long_n).unwrap();
    assert_eq!(
        sanitize_announce_msg(&a, pair, ValidationErr::InvalidCondition),
        Err(ValidationErr::InvalidCondition(long_n))
    );
}

#[cfg(test)]
fn amount_tester(buf: &[u8]) -> Result<u64, ValidationErr> {
    let mut a = Allocator::new();
    let n = a.new_atom(buf).unwrap();

    parse_amount(&a, n, ValidationErr::InvalidCoinAmount)
}

#[test]
fn test_sanitize_amount() {
    // negative amounts are not allowed
    assert!(
        matches!(
            amount_tester(&[0x80]).unwrap_err().1,
            ValidationErr::InvalidCoinAmount
        )
    );
    assert!(
        matches!(
            amount_tester(&[0xff]).unwrap_err().1,
            ValidationErr::InvalidCoinAmount
        )
    );
    assert!(
        matches!(
            amount_tester(&[0xff, 0]).unwrap_err().1,
            ValidationErr::InvalidCoinAmount
        )
    );

    // leading zeros are somtimes necessary to make values positive
    assert_eq!(amount_tester(&[0, 0xff]), Ok(0xff));
    // but are disallowed when they are redundant
    assert!(
        matches!(
            amount_tester(&[0, 0, 0, 0xff]).unwrap_err().1,
            ValidationErr::InvalidCoinAmount
        )
    );
    assert!(
        matches!(
            amount_tester(&[0, 0, 0, 0x80]).unwrap_err().1,
            ValidationErr::InvalidCoinAmount
        )
    );
    assert!(
        matches!(
            amount_tester(&[0, 0, 0, 0x7f]).unwrap_err().1,
            ValidationErr::InvalidCoinAmount
        )
    );
    assert!(
        matches!(
            amount_tester(&[0, 0, 0]).unwrap_err().1,
            ValidationErr::InvalidCoinAmount
        )
    );

    // amounts aren't allowed to be too big
    assert!(
        matches!(
            amount_tester(&[0x7f, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
            .unwrap_err()
            .1,
            ValidationErr::InvalidCoinAmount
        )
    );

    // this is small enough though
    assert_eq!(
        amount_tester(&[0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]),
        Ok(0xffff_ffff_ffff_ffff)
    );
}
