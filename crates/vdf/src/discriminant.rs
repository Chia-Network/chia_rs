//! Pure Rust implementation of create_discriminant using num_bigint.
//! Ported from src/create_discriminant.h and src/proof_common.h (HashPrime).

use num_bigint::{BigInt, Sign};
use num_traits::Zero;
use sha2::{Digest, Sha256};

/// Generates a pseudoprime using the hash-and-check method.
/// Randomly chooses x with bit-length `length`, applies bitmask (sets given bits to 1),
/// then returns x if it is a pseudoprime (Miller-Rabin), otherwise repeats.
/// Public for use by verifier GetB.
pub fn hash_prime(seed: &[u8], length_bits: u32, bitmask: &[u32]) -> BigInt {
    assert!(length_bits % 8 == 0, "length must be multiple of 8");
    let length_bytes = (length_bits / 8) as usize;

    let mut sprout = seed.to_vec();

    loop {
        let mut blob = Vec::with_capacity(length_bytes);

        while blob.len() < length_bytes {
            // Increment sprout by 1 (big-endian)
            let mut carry = 1u16;
            for i in (0..sprout.len()).rev() {
                let sum = sprout[i] as u16 + carry;
                sprout[i] = sum as u8;
                carry = sum >> 8;
                if carry == 0 {
                    break;
                }
            }
            if carry != 0 {
                sprout.insert(0, 1);
            }

            let mut hasher = Sha256::new();
            hasher.update(&sprout);
            let hash = hasher.finalize();
            let take = (length_bytes - blob.len()).min(hash.len());
            blob.extend_from_slice(&hash[..take]);
        }

        debug_assert!(blob.len() == length_bytes);

        // Build BigInt from blob (big-endian: first byte is MSB)
        let mut p = BigInt::from_bytes_be(Sign::Plus, &blob);

        // Apply bitmask: set bits to 1
        for &b in bitmask {
            p.set_bit(b as u64, true);
        }
        // Force odd
        p.set_bit(0, true);

        if is_prime_miller_rabin(&p) {
            return p;
        }
    }
}

/// Miller-Rabin primality test. For 1024-bit numbers, 20 rounds gives strong assurance.
fn is_prime_miller_rabin(n: &BigInt) -> bool {
    if n <= &BigInt::from(1) {
        return false;
    }
    if n <= &BigInt::from(3) {
        return true;
    }
    // Even?
    if !n.bit(0) {
        return false;
    }

    // Write n - 1 = 2^s * d
    let n_minus_1 = n - 1u32;
    let s = n_minus_1.trailing_zeros().unwrap_or(0) as usize;
    let d = &n_minus_1 >> s;

    // Bases used in Chia's BPSW / common strong tests for 64-bit and beyond
    const BASES: [u32; 12] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37];

    for base in BASES {
        let a = BigInt::from(base);
        if &a >= n {
            continue;
        }
        let mut x = modpow(&a, &d, n);
        if x == BigInt::from(1u32) || x == n_minus_1 {
            continue;
        }
        let mut cont = false;
        for _ in 1..s as u64 {
            x = (&x * &x) % n;
            if x == n_minus_1 {
                cont = true;
                break;
            }
        }
        if !cont {
            return false;
        }
    }
    true
}

/// Modular exponentiation: base^exp mod m
fn modpow(base: &BigInt, exp: &BigInt, m: &BigInt) -> BigInt {
    if exp.is_zero() {
        return BigInt::from(1);
    }
    let mut base = base % m;
    let mut exp = exp.clone();
    let mut result = BigInt::from(1);

    while !exp.is_zero() {
        if exp.bit(0) {
            result = (&result * &base) % m;
        }
        exp >>= 1;
        if !exp.is_zero() {
            base = (&base * &base) % m;
        }
    }
    result
}

/// Create discriminant from seed: HashPrime(seed, length, {0, 1, 2, length-1}) * -1.
/// Writes the discriminant as big-endian bytes into `result` (same as C mpz_export).
/// Returns true on success. `result.len() * 8` must equal `length_bits` (e.g. 1024 → 128 bytes).
pub fn create_discriminant(seed: &[u8], length_bits: u32, result: &mut [u8]) -> bool {
    let expected_bytes = (length_bits / 8) as usize;
    if result.len() != expected_bytes {
        return false;
    }

    let bitmask: Vec<u32> = vec![0, 1, 2, length_bits - 1];
    let p = hash_prime(seed, length_bits, &bitmask);
    let neg = -p;

    // Export as big-endian (order 1 in GMP terms), no padding
    let (_, bytes) = neg.to_bytes_be();
    if bytes.len() > result.len() {
        return false;
    }
    let offset = result.len() - bytes.len();
    result[..offset].fill(0);
    result[offset..].copy_from_slice(&bytes);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex;

    #[test]
    fn test_create_discriminant_known() {
        let seeds = [
            hex::decode("6c3b9aa767f785b537c0").unwrap(),
            hex::decode("b10da48cea4c09676b8e").unwrap(),
        ];
        let expected = [
            "9a8eaf9c52d9a5f1db648cdf7bcd04b35cb1ac4f421c978fa61fe1344b97d4199dbff700d24e7cfc0b785e4b8b8023dc49f0e90227f74f54234032ac3381879f",
            "b193cdb02f1c2615a257b98933ee0d24157ac5f8c46774d5d635022e6e6bd3f7372898066c2a40fa211d1df8c45cb95c02e36ef878bc67325473d9c0bb34b047",
        ];

        for (i, seed) in seeds.iter().enumerate() {
            let mut discriminant = vec![0u8; 128];
            assert!(create_discriminant(seed, 1024, &mut discriminant));
            assert_eq!(hex::encode(&discriminant), expected[i]);
        }
    }
}
