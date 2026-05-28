#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_consensus::sanitize_int::{SanitizedUint, sanitize_uint};
use chia_consensus::validation_error::{ErrorCode, ValidationErr};
use clvmr::allocator::Allocator;

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let atom = a.new_atom(data).unwrap();
    match sanitize_uint(
        &a,
        atom,
        8,
        ValidationErr::Err(ErrorCode::InvalidCoinAmount),
    ) {
        Ok(SanitizedUint::Ok(_)) => {
            assert!(data.len() <= 9);
            if data.len() == 9 {
                assert!(data[0] == 0);
            }
        }
        Ok(SanitizedUint::NegativeOverflow) => {
            assert!(!data.is_empty() && (data[0] & 0x80) != 0);
        }
        Ok(SanitizedUint::PositiveOverflow) => {
            assert!(data.len() > 8);
        }
        Err(ValidationErr::Err(c)) => {
            assert!(c == ErrorCode::InvalidCoinAmount);
        }
        _ => {
            panic!("invalid state");
        }
    }

    match sanitize_uint(
        &a,
        atom,
        4,
        ValidationErr::Err(ErrorCode::InvalidCoinAmount),
    ) {
        Ok(SanitizedUint::Ok(_)) => {
            assert!(data.len() <= 5);
            if data.len() == 5 {
                assert!(data[0] == 0);
            }
        }
        Ok(SanitizedUint::NegativeOverflow) => {
            assert!(!data.is_empty() && (data[0] & 0x80) != 0);
        }
        Ok(SanitizedUint::PositiveOverflow) => {
            assert!(data.len() > 4);
        }
        Err(ValidationErr::Err(c)) => {
            assert!(c == ErrorCode::InvalidCoinAmount);
        }
        _ => {
            panic!("invalid state");
        }
    }
});
