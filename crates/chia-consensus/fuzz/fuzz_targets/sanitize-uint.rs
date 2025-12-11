#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_consensus::sanitize_int::{sanitize_uint, SanitizedUint};
use chia_consensus::validation_error::ValidationErr;
use clvmr::allocator::Allocator;

fuzz_target!(|data: &[u8]| {
    let mut a = Allocator::new();
    let atom = a.new_atom(data).unwrap();
    match sanitize_uint(&a, atom, 8, ValidationErr::InvalidCoinAmount) {
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
        Err(ValidationErr::InvalidCoinAmount(n)) => {
            assert!(n == atom);
        }
        Err(e) => panic!("unexpected validation error: {:?}", e),
    }

    match sanitize_uint(&a, atom, 4, ValidationErr::InvalidCoinAmount) {
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
        Err(ValidationErr::InvalidCoinAmount(n)) => {
            assert!(n == atom);
        }
        Err(e) => panic!("unexpected validation error: {:?}", e),
    }
});
