#![allow(unsafe_code)]

use std::ptr;

use chia_vdf_verify_c::{
    ByteArray, create_discriminant_wrapper, delete_byte_array, prove_wrapper,
    verify_n_wesolowski_wrapper,
};

fn hex_decode(s: &str) -> Vec<u8> {
    hex::decode(s).unwrap()
}

/// Vectors from the chiavdf Rust crate bindings tests (512-bit discriminants).
#[test]
fn test_create_discriminant_matches_chiavdf() {
    let cases: &[(&str, &str)] = &[
        (
            "6c3b9aa767f785b537c0",
            "9a8eaf9c52d9a5f1db648cdf7bcd04b35cb1ac4f421c978fa61fe1344b97d4199dbff700d24e7cfc0b785e4b8b8023dc49f0e90227f74f54234032ac3381879f",
        ),
        (
            "b10da48cea4c09676b8e",
            "b193cdb02f1c2615a257b98933ee0d24157ac5f8c46774d5d635022e6e6bd3f7372898066c2a40fa211d1df8c45cb95c02e36ef878bc67325473d9c0bb34b047",
        ),
        (
            "c51b8a31c98b9fe13065",
            "bb5bd19ae50efe98b5ac56c69453a95e92dc16bb4b2824e73b39b9db0a077fa33fc2e775958af14f675a071bf53f1c22f90ccbd456e2291276951830dba9dcaf",
        ),
        (
            "5de9bc1bb4cb7a9f9cf9",
            "a1e93b8f2e9b0fd3b1325fbe40601f55e2afbdc6161409c0aff8737b7213d7d71cab21ffc83a0b6d5bdeee2fdcbbb34fbc8fc0b439915075afa9ffac8bb1b337",
        ),
        (
            "22cfaefc92e4edb9b0ae",
            "f2a10f70148fb30e4a16c4eda44cc0f9917cb9c2d460926d59a408318472e2cfd597193aa58e1fdccc6ae6a4d85bc9b27f77567ebe94fcedbf530a60ff709fd7",
        ),
    ];

    for (seed_hex, expected_hex) in cases {
        let seed = hex_decode(seed_hex);
        let mut result = [0u8; 64];
        // SAFETY: stack buffer and owned seed slice.
        let ok = unsafe {
            create_discriminant_wrapper(seed.as_ptr(), seed.len(), 512, result.as_mut_ptr())
        };
        assert!(ok, "create_discriminant_wrapper failed for seed {seed_hex}");
        assert_eq!(hex::encode(result), *expected_hex);
    }
}

#[test]
fn test_verify_n_wesolowski_known_proof() {
    let seed = b"test_seed_chia";
    let mut disc = [0u8; 64];
    // SAFETY: stack buffer and static seed.
    let ok =
        unsafe { create_discriminant_wrapper(seed.as_ptr(), seed.len(), 512, disc.as_mut_ptr()) };
    assert!(ok);

    let x_s = hex_decode(
        "08000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    );
    let proof = hex_decode(
        "020020417eb39c4e14954a817af644fc13d086c26dddab8afea12415b5e685f7883f5740ba01cb75220081c8aba7854cbd52010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    );

    // SAFETY: stack/owned buffers of correct sizes.
    let valid = unsafe {
        verify_n_wesolowski_wrapper(
            disc.as_ptr(),
            disc.len(),
            x_s.as_ptr(),
            proof.as_ptr(),
            proof.len(),
            100,
            0,
        )
    };
    assert!(valid, "known-good depth-0 proof must verify");
}

#[test]
fn test_verify_rejects_null_and_bad_input() {
    let disc = [1u8; 64];
    let x_s = [0u8; 100];
    let proof = [0u8; 200];

    // SAFETY: intentional null / valid pointers for negative tests.
    unsafe {
        assert!(!verify_n_wesolowski_wrapper(
            ptr::null(),
            64,
            x_s.as_ptr(),
            proof.as_ptr(),
            proof.len(),
            100,
            0
        ));
        assert!(!verify_n_wesolowski_wrapper(
            disc.as_ptr(),
            64,
            ptr::null(),
            proof.as_ptr(),
            proof.len(),
            100,
            0
        ));
        assert!(!verify_n_wesolowski_wrapper(
            disc.as_ptr(),
            64,
            x_s.as_ptr(),
            ptr::null(),
            proof.len(),
            100,
            0
        ));
        assert!(!create_discriminant_wrapper(
            ptr::null(),
            0,
            512,
            [0u8; 64].as_mut_ptr()
        ));
        assert!(!create_discriminant_wrapper(
            b"x".as_ptr(),
            1,
            512,
            ptr::null_mut()
        ));
        assert!(!create_discriminant_wrapper(
            b"x".as_ptr(),
            1,
            7,
            [0u8; 1].as_mut_ptr()
        ));
    }
}

#[test]
fn test_prove_stub_and_delete() {
    // SAFETY: prove is a stub; null args are fine.
    let array: ByteArray = unsafe { prove_wrapper(ptr::null(), 0, ptr::null(), 0, 1024, 1) };
    assert!(array.data.is_null());
    assert_eq!(array.length, 0);
    // SAFETY: null ByteArray is a no-op free.
    unsafe {
        delete_byte_array(array);
    }
}
