//! BigInt wrapper using malachite-nz with GMP-compatible operations.

use malachite_base::num::arithmetic::traits::{
    DivExact, DivMod, ExtendedGcd, FloorRoot, FloorSqrt, Gcd, JacobiSymbol, ModPow, Parity,
};
use malachite_base::num::basic::traits::Zero;
use malachite_base::num::logic::traits::SignificantBits;
use malachite_nz::integer::Integer;
use malachite_nz::natural::Natural;

pub use malachite_nz::integer::Integer as Int;

/// Number of bits in a machine limb (for Lehmer extraction).
pub const LIMB_BITS: usize = 64;

/// Compute floor division (matching GMP mpz_fdiv_q behavior).
/// GMP floors toward -infinity; malachite's DivMod also floors.
pub fn fdiv_q(a: &Integer, b: &Integer) -> Integer {
    a.div_mod(b).0
}

/// Compute floor remainder (matching GMP mpz_fdiv_r).
/// The remainder has the same sign as the divisor.
pub fn fdiv_r(a: &Integer, b: &Integer) -> Integer {
    let r = a % b;
    if r == 0i32 {
        r
    } else if (*b > 0i32) != (r > 0i32) {
        r + b
    } else {
        r
    }
}

/// Compute truncating division (matches Rust's default `/`).
pub fn tdiv_q(a: &Integer, b: &Integer) -> Integer {
    a / b
}

/// Compute truncating remainder (matches Rust's default `%`).
pub fn tdiv_r(a: &Integer, b: &Integer) -> Integer {
    a % b
}

/// Exact division — panics in debug if remainder != 0.
pub fn divexact(a: &Integer, b: &Integer) -> Integer {
    a.div_exact(b)
}

/// Integer square root (floor). Argument must be non-negative.
pub fn isqrt(n: &Integer) -> Integer {
    debug_assert!(*n >= 0i32, "isqrt: negative argument");
    Integer::from(n.unsigned_abs_ref().floor_sqrt())
}

/// nth root (floor). Argument must be non-negative.
pub fn nth_root(n: &Integer, k: u32) -> Integer {
    debug_assert!(*n >= 0i32, "nth_root: negative argument");
    Integer::from(n.unsigned_abs_ref().floor_root(k as u64))
}

/// Jacobi symbol (a/n). n must be odd and positive.
pub fn jacobi(a: &Integer, n: &Integer) -> i32 {
    debug_assert!(*n > 0i32, "n must be > 1, got {n}");
    debug_assert!(n.odd(), "n must be odd, got {n}");
    a.jacobi_symbol(n) as i32
}

/// modpow: base^exp mod modulus. base, exp, and modulus should be non-negative.
pub fn modpow(base: &Integer, exp: &Integer, modulus: &Integer) -> Integer {
    debug_assert!(*exp >= 0i32, "modpow: negative exponent");
    debug_assert!(*modulus > 0i32, "modpow: non-positive modulus");
    let base_nat = base.unsigned_abs_ref();
    let exp_nat = exp.unsigned_abs_ref();
    let mod_nat = modulus.unsigned_abs_ref();
    Integer::from(base_nat.clone().mod_pow(exp_nat.clone(), mod_nat))
}

/// Import signed big-endian bytes. Format: [sign_byte][magnitude_be...]
/// sign_byte: 0 = non-negative, 1 = negative.
/// Used for discriminant serialization to avoid decimal string parse overhead.
pub fn from_signed_bytes_be(bytes: &[u8]) -> Option<Integer> {
    if bytes.is_empty() {
        return None;
    }
    let sign = bytes[0] == 0; // true = non-negative, false = negative
    let mag = from_bytes_be(&bytes[1..]).unsigned_abs_ref().clone();
    Some(Integer::from_sign_and_abs(sign, mag))
}

/// Export as signed big-endian bytes. Format: [sign_byte][magnitude_be...]
pub fn to_signed_bytes_be(n: &Integer) -> Vec<u8> {
    let sign_byte: u8 = u8::from(*n < 0i32);
    let mut out = vec![sign_byte];
    out.extend_from_slice(&nat_to_bytes_be(n.unsigned_abs_ref()));
    out
}

/// Import big-endian bytes as a non-negative integer.
pub fn from_bytes_be(bytes: &[u8]) -> Integer {
    if bytes.is_empty() {
        return Integer::ZERO;
    }
    let num_limbs = bytes.len().div_ceil(8);
    let mut limbs = vec![0u64; num_limbs];
    for (i, &b) in bytes.iter().rev().enumerate() {
        limbs[i / 8] |= (b as u64) << ((i % 8) * 8);
    }
    Integer::from(Natural::from_owned_limbs_asc(limbs))
}

/// Export as big-endian bytes with the given byte length (zero-padded on left).
pub fn to_bytes_be_padded(n: &Integer, len: usize) -> Vec<u8> {
    let bytes = nat_to_bytes_be(n.unsigned_abs_ref());
    if bytes.len() >= len {
        bytes[bytes.len() - len..].to_vec()
    } else {
        let mut out = vec![0u8; len];
        out[len - bytes.len()..].copy_from_slice(&bytes);
        out
    }
}

/// Import little-endian bytes as a non-negative integer.
pub fn from_bytes_le(bytes: &[u8]) -> Integer {
    if bytes.is_empty() {
        return Integer::ZERO;
    }
    let num_limbs = bytes.len().div_ceil(8);
    let mut limbs = vec![0u64; num_limbs];
    for (i, &b) in bytes.iter().enumerate() {
        limbs[i / 8] |= (b as u64) << ((i % 8) * 8);
    }
    Integer::from(Natural::from_owned_limbs_asc(limbs))
}

/// Export as little-endian bytes with the given byte length (zero-padded on right).
pub fn to_bytes_le_padded(n: &Integer, len: usize) -> Vec<u8> {
    let nat = n.unsigned_abs_ref();
    let limbs = nat.as_limbs_asc();
    let mut out = vec![0u8; len];
    let mut pos = 0usize;
    'outer: for &limb in limbs {
        for i in 0..8 {
            if pos >= len {
                break 'outer;
            }
            out[pos] = (limb >> (i * 8)) as u8;
            pos += 1;
        }
    }
    out
}

/// Convert Natural to big-endian byte vector (no leading zeros except for zero itself).
fn nat_to_bytes_be(n: &Natural) -> Vec<u8> {
    let limbs = n.to_limbs_desc(); // most-significant limb first
    if limbs.is_empty() {
        return vec![];
    }
    let mut out = Vec::with_capacity(limbs.len() * 8);
    let mut leading = true;
    for limb in limbs {
        for i in (0..8).rev() {
            let byte = (limb >> (i * 8)) as u8;
            if leading && byte == 0 {
                continue;
            }
            leading = false;
            out.push(byte);
        }
    }
    out
}

/// Number of bits needed to represent the absolute value (minimum 1).
pub fn num_bits(n: &Integer) -> usize {
    if *n == 0i32 {
        return 1;
    }
    n.significant_bits() as usize
}

/// Extract a 64-bit signed integer and exponent approximation of n,
/// mirroring the C `mpz_get_si_2exp` used in the Reducer.
/// Returns (mantissa, exponent) where n ≈ mantissa * 2^(exponent - 63).
pub fn get_si_2exp(n: &Integer) -> (i64, i64) {
    if *n == 0i32 {
        return (0, 0);
    }
    let bits = num_bits(n) as i64;
    let shift = if bits > 64 { (bits - 64) as u64 } else { 0 };
    let shifted = n.unsigned_abs_ref() >> shift;
    let top_limb = shifted.as_limbs_asc().first().copied().unwrap_or(0);
    let lg2 = 64 - top_limb.leading_zeros() as i64;
    let mantissa_shift = 63 - lg2;
    let mantissa = if mantissa_shift >= 0 {
        (top_limb << mantissa_shift) as i64
    } else {
        (top_limb >> (-mantissa_shift)) as i64
    };
    let mantissa = if *n < 0i32 { -mantissa } else { mantissa };
    (mantissa, bits)
}

/// Extract the low word of (|x| >> shift_bits), where x is non-negative.
/// Optimized: avoids allocating a shifted copy by using direct limb indexing.
#[inline(always)]
pub fn extract_uword_from_shift_nonneg(x: &Integer, shift_bits: i64) -> i64 {
    let nat = x.unsigned_abs_ref();
    let limbs = nat.as_limbs_asc();
    if shift_bits <= 0 {
        return limbs.first().copied().unwrap_or(0) as i64;
    }
    let shift = shift_bits as u64;
    let limb_index = (shift / 64) as usize;
    let bit_offset = (shift % 64) as u32;
    let result = if bit_offset == 0 {
        limbs.get(limb_index).copied().unwrap_or(0)
    } else {
        let lo = limbs.get(limb_index).copied().unwrap_or(0);
        let hi = limbs.get(limb_index + 1).copied().unwrap_or(0);
        (lo >> bit_offset) | (hi << (64 - bit_offset))
    };
    result as i64
}

/// Get bit length of the absolute value (matching chiavdf_mpz_bitlen_nonneg).
pub fn bitlen_nonneg(x: &Integer) -> i64 {
    if *x == 0i32 {
        return 1;
    }
    x.significant_bits() as i64
}

/// Trailing zeros in the absolute value (number of factors of 2).
pub fn trailing_zeros(n: &Integer) -> u64 {
    n.trailing_zeros().unwrap_or(0)
}

/// Lehmer-accelerated full extended GCD via malachite's built-in implementation.
/// Returns (gcd, x, y) such that gcd = x * a + y * b.
pub fn fast_extended_gcd(a: &Integer, b: &Integer) -> (Integer, Integer, Integer) {
    let (gcd, x, y) = a.clone().extended_gcd(b.clone());
    (Integer::from(gcd), x, y)
}

/// Full extended GCD consuming both arguments (avoids cloning).
pub fn fast_extended_gcd_owned(a: Integer, b: Integer) -> (Integer, Integer, Integer) {
    let (gcd, x, y) = a.extended_gcd(b);
    (Integer::from(gcd), x, y)
}

/// Half extended GCD: returns (gcd, y) where gcd ≡ y * b (mod a).
/// Only tracks the coefficient of b.
pub fn fast_gcd_coeff_b(a: &Integer, b: &Integer) -> (Integer, Integer) {
    let (gcd, _x, y) = a.clone().extended_gcd(b.clone());
    (Integer::from(gcd), y)
}

/// Half extended GCD consuming both arguments.
pub fn fast_gcd_coeff_b_owned(a: Integer, b: Integer) -> (Integer, Integer) {
    let (gcd, _x, y) = a.extended_gcd(b);
    (Integer::from(gcd), y)
}

/// Half extended GCD returning (gcd, x) where gcd = x*a + y*b.
/// Only the first cofactor is returned. The expensive second cofactor
/// (computed via full-precision multiply + divide) is discarded.
pub fn fast_gcd_coeff_a_owned(a: Integer, b: Integer) -> (Integer, Integer) {
    let (gcd, x, _y) = a.extended_gcd(b);
    (Integer::from(gcd), x)
}

/// Compute gcd of two non-negative integers.
pub fn gcd_nonneg(a: &Integer, b: &Integer) -> Integer {
    Integer::from(
        a.unsigned_abs_ref()
            .clone()
            .gcd(b.unsigned_abs_ref().clone()),
    )
}

/// Extract the low 64 bits of (n >> shift) treating n as non-negative.
/// Used in the Lehmer inner loop.
/// Optimized: avoids allocating a shifted copy.
#[inline(always)]
pub fn extract_word_unsigned(n: &Integer, shift: usize) -> i64 {
    let nat = n.unsigned_abs_ref();
    let limbs = nat.as_limbs_asc();
    if shift == 0 {
        return limbs.first().copied().unwrap_or(0) as i64;
    }
    let limb_index = shift / 64;
    let bit_offset = (shift % 64) as u32;
    let result = if bit_offset == 0 {
        limbs.get(limb_index).copied().unwrap_or(0)
    } else {
        let lo = limbs.get(limb_index).copied().unwrap_or(0);
        let hi = limbs.get(limb_index + 1).copied().unwrap_or(0);
        (lo >> bit_offset) | (hi << (64 - bit_offset))
    };
    result as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_extended_gcd() {
        let cases: Vec<(i64, i64, i64)> = vec![
            (5, 3, 1),
            (6, 4, 2),
            (100, 37, 1),
            (15, 10, 5),
            (7, 5, 1),
            (35, 20, 5),
            (1_000_000, 999_983, 1),
            (0, 7, 7),
            (7, 0, 7),
            (1, 1, 1),
        ];
        for (a, b, expected_gcd) in cases {
            let ba = Integer::from(a);
            let bb = Integer::from(b);
            let (gcd, x, y) = fast_extended_gcd(&ba, &bb);
            assert_eq!(
                gcd,
                Integer::from(expected_gcd),
                "gcd({a},{b}) wrong: got {gcd}"
            );
            assert_eq!(
                &x * &ba + &y * &bb,
                gcd,
                "Bezout identity failed for ({a},{b}): {x}*{a}+{y}*{b}≠{gcd}"
            );
        }
        let (gcd, x, y) = fast_extended_gcd(&Integer::from(-15i64), &Integer::from(10i64));
        assert_eq!(gcd, Integer::from(5));
        assert_eq!(&x * Integer::from(-15i64) + &y * Integer::from(10i64), gcd);
    }

    #[test]
    fn test_jacobi() {
        assert_eq!(jacobi(&Integer::from(2), &Integer::from(7)), 1);
        assert_eq!(jacobi(&Integer::from(3), &Integer::from(7)), -1);
        assert_eq!(jacobi(&Integer::from(5), &Integer::from(9)), 1);
        assert_eq!(jacobi(&Integer::from(0), &Integer::from(7)), 0);
        assert_eq!(jacobi(&Integer::from(1), &Integer::from(7)), 1);
    }

    #[test]
    fn test_fdiv() {
        let a = Integer::from(-7i64);
        let b = Integer::from(2i64);
        assert_eq!(fdiv_q(&a, &b), Integer::from(-4i64));
        assert_eq!(fdiv_r(&a, &b), Integer::from(1i64));
    }

    #[test]
    fn test_isqrt() {
        assert_eq!(isqrt(&Integer::from(16)), Integer::from(4));
        assert_eq!(isqrt(&Integer::from(15)), Integer::from(3));
        assert_eq!(isqrt(&Integer::from(0)), Integer::from(0));
    }

    #[test]
    fn test_signed_bytes_roundtrip() {
        let cases = [
            Integer::ZERO,
            Integer::from(123i32),
            Integer::from(-123i32),
            Integer::from(0x1234_5678_i64),
            -Integer::from(0x1234_5678_i64),
        ];
        for n in cases {
            let bytes = to_signed_bytes_be(&n);
            let restored = from_signed_bytes_be(&bytes).expect("roundtrip failed");
            assert_eq!(n, restored, "roundtrip failed for {n}");
        }
    }
}
