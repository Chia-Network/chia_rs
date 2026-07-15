//! CreateDiscriminant: generate a negative discriminant from a seed.
//!
//! Port of chiavdf/src/create_discriminant.h.

use crate::primetest::hash_prime;
use malachite_nz::integer::Integer;

/// Create a discriminant D from a seed and bit length.
/// D = -HashPrime(seed, length, {0, 1, 2, length-1})
/// D ≡ 7 (mod 8), so D ≡ 1 (mod 8) after negation.
pub fn create_discriminant(seed: &[u8], length: usize) -> Integer {
    assert!(
        length > 0 && length.is_multiple_of(8),
        "length must be positive multiple of 8"
    );
    let bitmask = vec![0usize, 1, 2, length - 1];
    let p = hash_prime(seed, length, &bitmask);
    -p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integer::fdiv_r;
    use crate::primetest::is_prime_bpsw;

    #[test]
    fn test_discriminant_is_negative() {
        let seed = b"test_seed";
        let d = create_discriminant(seed, 512);
        assert!(d < 0i32, "discriminant should be negative");
    }

    #[test]
    fn test_discriminant_mod8() {
        let seed = b"test_seed";
        let d = create_discriminant(seed, 512);
        let r = fdiv_r(&d, &Integer::from(8i32));
        assert_eq!(r, Integer::from(1i32), "discriminant should be ≡ 1 mod 8");
    }

    #[test]
    fn test_discriminant_is_prime_magnitude() {
        let seed = b"small_test";
        let d = create_discriminant(seed, 256);
        assert!(
            is_prime_bpsw(&(-d)),
            "discriminant magnitude should be prime"
        );
    }
}
