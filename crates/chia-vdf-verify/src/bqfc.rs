//! BQFC compressed form serialization/deserialization.
//!
//! Port of chiavdf/src/bqfc.c and bqfc.h.
//!
//! Format (100 bytes for 1024-bit discriminant):
//!   Byte 0: flags (b_sign, t_sign, is_identity, is_generator)
//!   Byte 1: g_size (size of 'g' in bytes minus 1)
//!   d/16 - g_size bytes: a' = a/g
//!   d/32 - g_size bytes: t' = t/g
//!   g_size+1 bytes: g
//!   g_size+1 bytes: b0

use crate::integer::{
    divexact, fdiv_r, from_bytes_le, gcd_nonneg, isqrt, num_bits, tdiv_q, to_bytes_le_padded,
};
use crate::xgcd_partial::xgcd_partial;
use malachite_base::num::arithmetic::traits::{ExtendedGcd, FloorSqrt, ModPow};
use malachite_base::num::basic::traits::{One, Zero};
use malachite_base::num::logic::traits::SignificantBits;
use malachite_nz::integer::Integer;
use malachite_nz::natural::Natural;

/// Size of the serialized form (100 bytes for 1024-bit max discriminant).
pub const BQFC_FORM_SIZE: usize = 1024_usize.div_ceil(32) * 3 + 4;

/// Flag bits
const BQFC_B_SIGN: u8 = 1 << 0;
const BQFC_T_SIGN: u8 = 1 << 1;
const BQFC_IS_1: u8 = 1 << 2;
const BQFC_IS_GEN: u8 = 1 << 3;

/// Compressed form intermediate representation.
struct QfbC {
    a: Integer,
    t: Integer,
    g: Integer,
    b0: Integer,
    b_sign: bool,
}

/// Compress (a, b) to intermediate representation.
fn bqfc_compr(a: &Integer, b: &Integer) -> QfbC {
    if a == b {
        return QfbC {
            a: a.clone(),
            t: Integer::ZERO,
            g: Integer::ZERO,
            b0: Integer::ZERO,
            b_sign: false,
        };
    }

    let sign = *b < 0i32;
    let a_sqrt = isqrt(a);

    let mut a_copy = a.clone();
    let mut b_copy = if sign { -b } else { b.clone() };

    let mut dummy = Integer::ZERO;
    let mut t = Integer::ZERO;

    xgcd_partial(&mut dummy, &mut t, &mut a_copy, &mut b_copy, &a_sqrt);
    t = -t;

    let g = gcd_nonneg(a, &t);

    let (a_out, b0) = if g == 1i32 {
        (a.clone(), Integer::ZERO)
    } else {
        let a_new = divexact(a, &g);
        let t_new = divexact(&t, &g);
        let b0 = tdiv_q(b, &a_new);
        let b0 = if sign { -b0 } else { b0 };
        t = t_new;
        (a_new, b0)
    };

    QfbC {
        a: a_out,
        t,
        g,
        b_sign: sign,
        b0,
    }
}

/// Decompress intermediate representation to (a, b) given discriminant D.
fn bqfc_decompr(d: &Integer, c: &QfbC) -> Result<(Integer, Integer), String> {
    if c.t == 0i32 {
        return Ok((c.a.clone(), c.a.clone()));
    }

    let t = if c.t < 0i32 { &c.t + &c.a } else { c.t.clone() };

    if c.a == 0i32 {
        return Err("bqfc_decompr: a is zero".to_string());
    }

    // Compute modular inverse of t mod a using extended GCD
    let t_nat = t.unsigned_abs_ref().clone();
    let a_nat = c.a.unsigned_abs_ref().clone();
    let (gcd, x, _y) = t_nat.extended_gcd(a_nat.clone());
    if gcd != Natural::ONE {
        return Err(format!("bqfc_decompr: gcd(t, a) = {} != 1", gcd));
    }
    let t_inv = if x < Integer::ZERO { x + &c.a } else { x };

    let d_mod_a = fdiv_r(d, &c.a);

    // t^2 mod a — squaring is sign-agnostic, use |c.t|
    let ct_nat = c.t.unsigned_abs_ref().clone();
    let a_nat2 = c.a.unsigned_abs_ref().clone();
    let t_sq_mod = Integer::from(ct_nat.mod_pow(Natural::from(2u32), a_nat2));

    let tmp_prod = &t_sq_mod * &d_mod_a;
    let tmp_mod = &tmp_prod % &c.a;

    let tmp_mod_nat = tmp_mod.unsigned_abs_ref().clone();
    let tmp_sqrt = Integer::from((&tmp_mod_nat).floor_sqrt());
    if &tmp_sqrt * &tmp_sqrt != tmp_mod {
        return Err("bqfc_decompr: not a perfect square".to_string());
    }

    let out_b = (&tmp_sqrt * &t_inv) % &c.a;

    let out_a = if c.g > 1i32 { &c.a * &c.g } else { c.a.clone() };

    let out_b = if c.b0 > 0i32 {
        out_b + &c.a * &c.b0
    } else {
        out_b
    };

    let out_b = if c.b_sign { -out_b } else { out_b };

    Ok((out_a, out_b))
}

/// Export n as little-endian bytes of exactly `size` bytes.
fn export_le(out: &mut Vec<u8>, n: &Integer, size: usize) {
    let bytes = to_bytes_le_padded(n, size);
    out.extend_from_slice(&bytes);
}

/// Serialize (a, b) with discriminant bit length d_bits to 100-byte output.
pub fn serialize(a: &Integer, b: &Integer, d_bits: usize) -> Vec<u8> {
    let mut out = vec![0u8; BQFC_FORM_SIZE];

    if *b == 1i32 && *a <= 2i32 {
        out[0] = if *a == 2i32 { BQFC_IS_GEN } else { BQFC_IS_1 };
        return out;
    }

    let d_bits_rounded = (d_bits + 31) & !31usize;
    let c = bqfc_compr(a, b);
    let valid_size = bqfc_get_compr_size(d_bits);

    let mut buf = Vec::with_capacity(valid_size);
    let mut flags = 0u8;
    if c.b_sign {
        flags |= BQFC_B_SIGN;
    }
    if c.t < 0i32 {
        flags |= BQFC_T_SIGN;
    }

    let g_size = if c.g == 0i32 {
        0usize
    } else {
        let bits = c.g.significant_bits() as usize;
        bits.div_ceil(8)
    };
    let g_size = if g_size == 0 { 0 } else { g_size - 1 };

    buf.push(flags);
    buf.push(g_size as u8);

    export_le(&mut buf, &c.a, d_bits_rounded / 16 - g_size);
    let t_abs = if c.t < 0i32 {
        -c.t.clone()
    } else {
        c.t.clone()
    };
    export_le(&mut buf, &t_abs, d_bits_rounded / 32 - g_size);
    export_le(&mut buf, &c.g, g_size + 1);
    export_le(&mut buf, &c.b0, g_size + 1);

    let copy_len = buf.len().min(BQFC_FORM_SIZE);
    out[..copy_len].copy_from_slice(&buf[..copy_len]);
    out
}

/// Deserialize a form from 100-byte input with discriminant D.
/// Returns (a, b) or error string.
pub fn deserialize(d: &Integer, data: &[u8]) -> Result<(Integer, Integer), String> {
    if data.len() != BQFC_FORM_SIZE {
        return Err(format!(
            "expected {} bytes, got {}",
            BQFC_FORM_SIZE,
            data.len()
        ));
    }

    if data[0] & (BQFC_IS_1 | BQFC_IS_GEN) != 0 {
        let a = if data[0] & BQFC_IS_GEN != 0 {
            Integer::from(2u32)
        } else {
            Integer::ONE
        };
        return Ok((a, Integer::ONE));
    }

    let d_bits = num_bits(d);
    let d_bits_rounded = (d_bits + 31) & !31usize;

    let g_size = data[1] as usize;
    if g_size >= d_bits_rounded / 32 {
        return Err("g_size out of range".to_string());
    }

    let mut offset = 2usize;

    let a_bytes = d_bits_rounded / 16 - g_size;
    let a = from_bytes_le(&data[offset..offset + a_bytes]);
    offset += a_bytes;

    let t_bytes = d_bits_rounded / 32 - g_size;
    let t_raw = from_bytes_le(&data[offset..offset + t_bytes]);
    offset += t_bytes;

    let g = from_bytes_le(&data[offset..offset + g_size + 1]);
    offset += g_size + 1;

    let b0 = from_bytes_le(&data[offset..offset + g_size + 1]);

    let b_sign = data[0] & BQFC_B_SIGN != 0;
    let t_sign = data[0] & BQFC_T_SIGN != 0;
    let t = if t_sign { -t_raw } else { t_raw };

    let c = QfbC {
        a,
        t,
        g,
        b0,
        b_sign,
    };

    let (out_a, out_b) = bqfc_decompr(d, &c)?;

    let canon = serialize(&out_a, &out_b, d_bits);
    if canon != data {
        return Err("non-canonical serialization".to_string());
    }

    Ok((out_a, out_b))
}

/// Compute the serialization size for a given discriminant bit length.
pub fn bqfc_get_compr_size(d_bits: usize) -> usize {
    let d_bits_rounded = (d_bits + 31) & !31usize;
    d_bits_rounded / 32 * 3 + 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bqfc_form_size() {
        assert_eq!(bqfc_get_compr_size(1024), 100);
        assert_eq!(BQFC_FORM_SIZE, 100);
    }

    #[test]
    fn test_serialize_identity() {
        let d = Integer::from(-47i64);
        let a = Integer::ONE;
        let b = Integer::ONE;
        let data = serialize(&a, &b, num_bits(&d));
        assert_eq!(data[0], BQFC_IS_1);
    }

    #[test]
    fn test_serialize_generator() {
        let d = Integer::from(-47i64);
        let a = Integer::from(2i32);
        let b = Integer::ONE;
        let data = serialize(&a, &b, num_bits(&d));
        assert_eq!(data[0], BQFC_IS_GEN);
    }

    #[test]
    fn test_bqfc_roundtrip_identity() {
        let d = Integer::from(-47i64);
        let d_bits = num_bits(&d);
        let a = Integer::ONE;
        let b = Integer::ONE;
        let data = serialize(&a, &b, d_bits);
        let (ra, rb) = deserialize(&d, &data).unwrap();
        assert_eq!(ra, a);
        assert_eq!(rb, b);
    }
}
