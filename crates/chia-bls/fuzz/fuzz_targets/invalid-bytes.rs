#![no_main]
use libfuzzer_sys::fuzz_target;

use chia_bls::{PublicKey, SecretKey, Signature};

fuzz_target!(|data: &[u8]| {
    // It will be an error if the data is less than 32 bytes
    assert_eq!(SecretKey::from_seed(data).is_ok(), data.len() >= 32);

    if let Ok(bytes) = <[u8; 32]>::try_from(data) {
        SecretKey::from_bytes(&bytes).ok();
    }

    if let Ok(bytes) = <[u8; 48]>::try_from(data) {
        PublicKey::from_bytes(&bytes).ok();
    }

    if let Ok(bytes) = <[u8; 96]>::try_from(data) {
        Signature::from_bytes(&bytes).ok();
    }
});
