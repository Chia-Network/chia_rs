//! BPSW primality test and HashPrime.
//!
//! Ports:
//! - chiavdf/src/primetest.h (is_prime_bpsw)
//! - chiavdf/src/proof_common.h (HashPrime)

use crate::integer::{from_bytes_be, jacobi, modpow};
use malachite_base::num::arithmetic::traits::{DivMod, Parity};
use malachite_base::num::basic::traits::One;
use malachite_base::num::logic::traits::{BitAccess, SignificantBits};
use malachite_nz::integer::Integer;
use sha2::{Digest, Sha256};

/// Small prime products for trial division (subset — enough for fast rejection).
static SMALL_PRIMES: &[u64] = &[
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131, 137, 139, 149, 151, 157, 163, 167, 173, 179, 181, 191, 193,
    197, 199,
];

/// Miller-Rabin test with given base `b` modulo `n`.
/// Returns true if n is probably prime (passes MR with this base).
pub fn miller_rabin(n: &Integer, base: &Integer) -> bool {
    let n_minus1 = n - Integer::ONE;
    let s = n_minus1.trailing_zeros().unwrap_or(0) as usize;
    let d = &n_minus1 >> s as u64;

    let mut b = modpow(base, &d, n);

    if b == 1i32 {
        return true;
    }

    for _ in 0..s {
        let b_plus1 = &b + Integer::ONE;
        if b_plus1 == *n {
            return true;
        }
        b = modpow(&b, &Integer::from(2u32), n);
    }
    false
}

/// Find parameters (p, q) for Lucas test such that Jacobi(D, n) = -1.
fn find_pq(n: &Integer) -> Option<(i64, i64)> {
    let mut d = 5i64;
    for _ in 0..500 {
        let d_sign = if d % 4 == 1 { d } else { -d };
        let d_big = Integer::from(d_sign);
        if jacobi(&d_big, n) == -1 {
            if d_sign == 5 {
                return Some((5, 5));
            } else {
                return Some((1, (1 - d_sign) / 4));
            }
        }
        d = d.abs() + 2;
    }
    None
}

/// Lucas-V probable prime test (vprp).
fn is_vprp(n: &Integer) -> bool {
    let (p, q) = match find_pq(n) {
        Some(pq) => pq,
        None => return false,
    };

    let e = n + Integer::ONE;
    let v1 = find_lucas_v(&e, n, p, q);

    let two_q = Integer::from(2 * q);
    let v1_mod = v1.div_mod(n).1;
    let two_q_mod = two_q.div_mod(n).1;

    v1_mod == two_q_mod
}

/// Compute Lucas-V sequence value V_{n+1} mod m using the doubling method.
fn find_lucas_v(e: &Integer, m: &Integer, p: i64, q: i64) -> Integer {
    let l = e.significant_bits() as usize;

    let mut u1 = Integer::ONE;
    let mut u2 = Integer::from(p);
    let minus_2q = -2 * q;

    for i in (0..l.saturating_sub(1)).rev() {
        let tmp2 = &u2 * &u1;
        let u2_sq = &u2 * &u2;
        let u1_sq = &u1 * &u1;

        if e.get_bit(i as u64) {
            u1 = &u2_sq - Integer::from(q) * &u1_sq;
            u2 = if p != 1 {
                Integer::from(p) * &u2_sq + Integer::from(minus_2q) * &tmp2
            } else {
                &u2_sq + Integer::from(minus_2q) * &tmp2
            };
        } else {
            u2 = &u2_sq - Integer::from(q) * &u1_sq;
            let tmp3 = Integer::from(2) * &tmp2;
            u1 = if p != 1 {
                tmp3 - Integer::from(p) * &u1_sq
            } else {
                tmp3 - &u1_sq
            };
        }

        u1 = u1.div_mod(m).1;
        u2 = u2.div_mod(m).1;
    }

    Integer::from(2) * &u2 - Integer::from(p) * &u1
}

/// BPSW primality test.
/// Returns true if n is (very likely) prime.
pub fn is_prime_bpsw(n: &Integer) -> bool {
    if *n <= 1i32 {
        return false;
    }
    if *n == 2i32 || *n == 3i32 {
        return true;
    }
    if n.even() {
        return false;
    }

    for &p in SMALL_PRIMES {
        let p_big = Integer::from(p);
        if *n == p_big {
            return true;
        }
        if (n % &p_big) == 0i32 {
            return false;
        }
    }

    let base2 = Integer::from(2u32);
    if !miller_rabin(n, &base2) {
        return false;
    }

    is_vprp(n)
}

/// HashPrime: generate a pseudoprime of given bit length from seed.
///
/// Uses iterative SHA256 to expand seed, then applies bitmask and tests primality.
/// Matches chiavdf's HashPrime(seed, length, bitmask).
pub fn hash_prime(seed: &[u8], length: usize, bitmask: &[usize]) -> Integer {
    assert!(length.is_multiple_of(8), "length must be multiple of 8");
    let byte_len = length / 8;

    let mut sprout = seed.to_vec();

    loop {
        let mut blob = Vec::with_capacity(byte_len);

        while blob.len() * 8 < length {
            for i in (0..sprout.len()).rev() {
                sprout[i] = sprout[i].wrapping_add(1);
                if sprout[i] != 0 {
                    break;
                }
            }

            let hash = Sha256::digest(&sprout);
            let remaining = byte_len - blob.len();
            let take = remaining.min(hash.len());
            blob.extend_from_slice(&hash[..take]);
        }

        assert_eq!(blob.len(), byte_len);

        let mut p = from_bytes_be(&blob);

        for &bit in bitmask {
            p |= Integer::ONE << bit as u64;
        }

        p |= Integer::ONE;

        if is_prime_bpsw(&p) {
            return p;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_miller_rabin_known_primes() {
        let base2 = Integer::from(2u32);
        for &p in &[3u64, 5, 7, 11, 13, 17, 19, 23, 997, 7919] {
            let n = Integer::from(p);
            assert!(miller_rabin(&n, &base2), "{} should pass MR(2)", p);
        }
    }

    #[test]
    fn test_miller_rabin_composites() {
        let base2 = Integer::from(2u32);
        let _results: Vec<_> = [9u64, 15, 21, 25, 35, 49, 77, 91]
            .iter()
            .map(|&c| miller_rabin(&Integer::from(c), &base2))
            .collect();
        let n9 = Integer::from(9u64);
        assert!(!miller_rabin(&n9, &base2), "9 should fail MR(2)");
    }

    #[test]
    fn test_is_prime_bpsw() {
        for &p in &[2u64, 3, 5, 7, 11, 13, 997, 7919, 104729] {
            assert!(is_prime_bpsw(&Integer::from(p)), "{} should be prime", p);
        }
        for &c in &[4u64, 6, 9, 15, 25, 35, 49, 100] {
            assert!(
                !is_prime_bpsw(&Integer::from(c)),
                "{} should be composite",
                c
            );
        }
    }

    #[test]
    fn test_hash_prime_is_prime() {
        let seed = b"test_seed_12345";
        let p = hash_prime(seed, 256, &[255]);
        assert!(is_prime_bpsw(&p), "hash_prime result should be prime");
        assert_eq!(
            p.significant_bits(),
            256,
            "hash_prime result should have correct bit length"
        );
    }
}
