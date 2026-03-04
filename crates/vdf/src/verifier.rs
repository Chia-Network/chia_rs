//! Pure Rust implementation of verify_n_wesolowski using num_bigint.
//! Ported from src/verifier.h, proof_common.h, bqfc.c, nucomp.h, vdf_new.h.

use num_bigint::{BigInt, Sign};
use num_traits::{Signed, Zero};

use crate::pure::discriminant;

const B_BITS: u32 = 264;
const B_BYTES: usize = (B_BITS as usize + 7) / 8; // 33
const BQFC_FORM_SIZE: usize = ((1024 + 31) / 32) * 3 + 4; // 100

const BQFC_B_SIGN: u8 = 1 << 0;
const BQFC_T_SIGN: u8 = 1 << 1;
const BQFC_IS_1: u8 = 1 << 2;
const BQFC_IS_GEN: u8 = 1 << 3;

/// Binary quadratic form (a, b, c): ax^2 + bxy + cy^2. Invariant: b^2 - 4ac = D.
#[derive(Clone)]
struct Form {
    a: BigInt,
    b: BigInt,
    c: BigInt,
}

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

/// Integer 4th root (floor).
fn root4(n: &BigInt) -> BigInt {
    if n <= &BigInt::from(1) {
        return n.clone();
    }
    let mut lo = BigInt::from(1);
    let mut hi = n.clone();
    while &lo + 1 < hi {
        let mid = (&lo + &hi) >> 1;
        let mid4 = &mid * &mid * &mid * &mid;
        if mid4 <= *n {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

fn normalize(a: &mut BigInt, b: &mut BigInt, c: &mut BigInt) {
    let (a_val, b_val, c_val) = (a.clone(), b.clone(), c.clone());
    let two_a = &a_val << 1;
    let r = (&a_val - &b_val) / &two_a;
    *a = a_val.clone();
    *b = &b_val + (&r * &a_val << 1);
    *c = &a_val * &r * &r + &b_val * &r + &c_val;
}

fn reduce_impl(a: &mut BigInt, b: &mut BigInt, c: &mut BigInt) {
    let (a_val, b_val, c_val) = (a.clone(), b.clone(), c.clone());
    let two_c = &c_val << 1;
    let s = (&c_val + &b_val) / &two_c;
    *a = c_val.clone();
    *b = (&s * &c_val << 1) - &b_val;
    *c = &c_val * &s * &s - &b_val * &s + &a_val;
}

fn form_reduce(a: &mut BigInt, b: &mut BigInt, c: &mut BigInt) {
    normalize(a, b, c);
    loop {
        let need = a > c || (a == c && b.sign() == Sign::Minus);
        if !need {
            break;
        }
        if a > c {
            std::mem::swap(a, c);
            *b = -b.clone();
        } else if a == c && b.sign() == Sign::Minus {
            *b = -b.clone();
        }
        reduce_impl(a, b, c);
    }
    normalize(a, b, c);
}

impl Form {
    fn from_abd(a: BigInt, b: BigInt, d: &BigInt) -> Result<Form, ()> {
        if a <= BigInt::from(0) {
            return Err(());
        }
        let b2 = &b * &b;
        let diff = &b2 - d;
        let four_a = &a << 2;
        if &diff % &four_a != BigInt::from(0) {
            return Err(());
        }
        let mut c = diff / four_a;
        let mut a = a;
        let mut b = b;
        form_reduce(&mut a, &mut b, &mut c);
        Ok(Form { a, b, c })
    }

    fn identity(d: &BigInt) -> Form {
        Form::from_abd(BigInt::from(1), BigInt::from(1), d).unwrap()
    }

    fn reduce(&mut self) {
        form_reduce(&mut self.a, &mut self.b, &mut self.c);
    }
}

fn gcd(a: &BigInt, b: &BigInt) -> BigInt {
    let mut a = a.abs();
    let mut b = b.abs();
    while !b.is_zero() {
        let t = b.clone();
        b = &a % &b;
        a = t;
    }
    a
}

fn isqrt(n: &BigInt) -> BigInt {
    if n <= &BigInt::from(0) {
        return BigInt::from(0);
    }
    let mut x = n.clone();
    let mut y = (&x + 1) >> 1;
    while y < x {
        x = y;
        y = (x.clone() + n / &x) >> 1;
    }
    x
}

fn extended_gcd(a: &BigInt, b: &BigInt) -> (BigInt, BigInt, BigInt) {
    let (mut s, mut old_s) = (BigInt::from(0), BigInt::from(1));
    let (mut t, mut old_t) = (BigInt::from(1), BigInt::from(0));
    let (mut r, mut old_r) = (b.clone(), a.clone());
    while !r.is_zero() {
        let q = &old_r / &r;
        let (new_r, new_s, new_t) = (
            &old_r - &q * &r,
            &old_s - &q * &s,
            &old_t - &q * &t,
        );
        old_r = r;
        old_s = s;
        old_t = t;
        r = new_r;
        s = new_s;
        t = new_t;
    }
    (old_r, old_s, old_t)
}

fn xgcd_partial_simple(
    co2: &mut BigInt,
    co1: &mut BigInt,
    r2: &mut BigInt,
    r1: &mut BigInt,
    l: &BigInt,
) {
    *co2 = BigInt::from(0);
    *co1 = BigInt::from(-1);
    while !r1.is_zero() && r1.abs() > *l {
        let q = &*r2 / &*r1;
        let rem = &*r2 % r1.clone();
        *r2 = r1.clone();
        *r1 = rem;
        let new_co2 = co2.clone() - &*co1 * &q;
        *co2 = std::mem::replace(co1, new_co2);
    }
    if r2.sign() == Sign::Minus {
        *r2 = -r2.clone();
        *co2 = -co2.clone();
        *co1 = -co1.clone();
    }
}

fn bqfc_compr(a: &BigInt, b: &BigInt) -> Result<(BigInt, BigInt, BigInt, BigInt, bool), ()> {
    if a == b {
        return Ok((a.clone(), BigInt::from(0), BigInt::from(0), BigInt::from(0), false));
    }
    let a_sqrt = isqrt(a);
    let mut a_copy = a.clone();
    let mut b_copy = b.clone();
    let sign = b_copy.sign() == Sign::Minus;
    if sign {
        b_copy = -b_copy;
    }
    let mut dummy = BigInt::from(0);
    let mut t = BigInt::from(0);
    xgcd_partial_simple(&mut dummy, &mut t, &mut a_copy, &mut b_copy, &a_sqrt);
    t = -t;

    let g = gcd(a, &t);
    let (a_out, t_out, b0) = if g == BigInt::from(1) {
        (a.clone(), BigInt::from(0), BigInt::from(0))
    } else {
        let a_out = a / &g;
        let t_out = &t / &g;
        let b0 = if sign { -(b / &a_out) } else { b / &a_out };
        (a_out, t_out, b0)
    };
    Ok((a_out, t_out, g, b0, sign))
}

fn is_perfect_square(n: &BigInt) -> Option<BigInt> {
    if n < &BigInt::from(0) {
        return None;
    }
    let r = isqrt(n);
    if &r * &r == *n {
        Some(r)
    } else {
        None
    }
}

fn bqfc_decompr(
    d: &BigInt,
    a: &BigInt,
    t: &BigInt,
    g: &BigInt,
    b0: &BigInt,
    b_sign: bool,
) -> Result<(BigInt, BigInt), ()> {
    if t.is_zero() {
        return Ok((a.clone(), a.clone()));
    }
    let mut t_pos = t.clone();
    if t_pos.sign() == Sign::Minus {
        t_pos = &t_pos + a;
    }
    if a.is_zero() {
        return Err(());
    }
    let (_, t_inv, _) = extended_gcd(&t_pos, a);
    let mut t_inv = t_inv;
    if t_inv.sign() == Sign::Minus {
        t_inv = &t_inv + a;
    }
    let d_mod = d % a;
    let tmp = (t * t % a) * &d_mod % a;
    let tmp_sqrt = is_perfect_square(&tmp).ok_or(())?;
    let mut out_b = (&tmp_sqrt * &t_inv) % a;
    let out_a = if g > &BigInt::from(1) {
        a * g
    } else {
        a.clone()
    };
    if b0 > &BigInt::from(0) {
        out_b = out_b + a * b0;
    }
    if b_sign {
        out_b = -out_b;
    }
    Ok((out_a, out_b))
}

fn bqfc_get_compr_size(d_bits: usize) -> usize {
    ((d_bits + 31) / 32) * 3 + 4
}

fn export_be(buf: &mut [u8], n: &BigInt) {
    let (_, bytes): (_, Vec<u8>) = n.abs().to_bytes_be();
    let start = buf.len().saturating_sub(bytes.len());
    buf[..start].fill(0);
    let len = (buf.len() - start).min(bytes.len());
    buf[start..start + len].copy_from_slice(&bytes[bytes.len() - len..]);
}

fn import_be(buf: &[u8]) -> BigInt {
    BigInt::from_bytes_be(Sign::Plus, buf)
}

fn bqfc_serialize_only(
    out: &mut [u8],
    a: &BigInt,
    t: &BigInt,
    g: &BigInt,
    b0: &BigInt,
    b_sign: bool,
    t_sign: bool,
    d_bits: usize,
) {
    let d_bits = (d_bits + 31) & !31;
    out[0] = (b_sign as u8) * BQFC_B_SIGN | (t_sign as u8) * BQFC_T_SIGN;
    let g_bits = g.bits();
    let g_size = ((g_bits + 7) / 8).max(1) as usize - 1;
    out[1] = g_size as u8;
    let mut offset = 2;
    let sz_a = d_bits / 16 - g_size;
    let sz_t = d_bits / 32 - g_size;
    let sz_g = g_size + 1;
    export_be(&mut out[offset..offset + sz_a], a);
    offset += sz_a;
    export_be(&mut out[offset..offset + sz_t], t);
    offset += sz_t;
    export_be(&mut out[offset..offset + sz_g], g);
    offset += sz_g;
    export_be(&mut out[offset..offset + sz_g], b0);
}

fn bqfc_serialize(form: &Form, d_bits: usize, out: &mut [u8]) -> Result<(), ()> {
    if form.b == BigInt::from(1) && form.a <= BigInt::from(2) {
        out[0] = if form.a == BigInt::from(2) {
            BQFC_IS_GEN
        } else {
            BQFC_IS_1
        };
        out[1..].fill(0);
        return Ok(());
    }
    let (a, t, g, b0, b_sign) = bqfc_compr(&form.a, &form.b)?;
    let t_sign = t.sign() == Sign::Minus;
    bqfc_serialize_only(out, &a, &t, &g, &b0, b_sign, t_sign, d_bits);
    let valid_size = bqfc_get_compr_size(d_bits);
    if valid_size < BQFC_FORM_SIZE {
        out[valid_size..BQFC_FORM_SIZE].fill(0);
    }
    Ok(())
}

fn bqfc_deserialize(d: &BigInt, bytes: &[u8], d_bits: usize) -> Result<Form, ()> {
    if bytes.len() != BQFC_FORM_SIZE {
        return Err(());
    }
    if bytes[0] & (BQFC_IS_1 | BQFC_IS_GEN) != 0 {
        let a = if bytes[0] & BQFC_IS_GEN != 0 {
            BigInt::from(2)
        } else {
            BigInt::from(1)
        };
        return Form::from_abd(a, BigInt::from(1), d);
    }
    let d_bits = (d_bits + 31) & !31;
    let g_size = bytes[1] as usize;
    if g_size >= d_bits / 32 {
        return Err(());
    }
    let mut offset = 2;
    let sz_a = d_bits / 16 - g_size;
    let a = import_be(&bytes[offset..offset + sz_a]);
    offset += sz_a;
    let sz_t = d_bits / 32 - g_size;
    let mut t = import_be(&bytes[offset..offset + sz_t]);
    offset += sz_t;
    if bytes[0] & BQFC_T_SIGN != 0 {
        t = -t;
    }
    let sz_g = g_size + 1;
    let g = import_be(&bytes[offset..offset + sz_g]);
    offset += sz_g;
    let b0 = import_be(&bytes[offset..offset + sz_g]);
    let b_sign = bytes[0] & BQFC_B_SIGN != 0;
    let (out_a, out_b) = bqfc_decompr(d, &a, &t, &g, &b0, b_sign)?;
    Form::from_abd(out_a, out_b, d)
}

fn fast_pow(exp: u64, m: &BigInt) -> BigInt {
    modpow(&BigInt::from(2), &BigInt::from(exp), m)
}

fn get_b(d: &BigInt, x: &Form, y: &Form) -> BigInt {
    let d_bits = d.bits() as usize;
    let mut ser = vec![0u8; BQFC_FORM_SIZE * 2];
    bqfc_serialize(x, d_bits, &mut ser[..BQFC_FORM_SIZE]).unwrap();
    bqfc_serialize(y, d_bits, &mut ser[BQFC_FORM_SIZE..]).unwrap();
    discriminant::hash_prime(&ser, B_BITS, &[B_BITS - 1])
}

fn compute_l(d: &BigInt) -> BigInt {
    root4(&d.abs())
}

/// Form exponentiation: form^exp in class group (using nucomp/nudupl).
fn fast_pow_form(x: &Form, d: &BigInt, num_iterations: &BigInt, l: &BigInt) -> Form {
    if num_iterations.is_zero() {
        return Form::identity(d);
    }
    let mut res = x.clone();
    let nbits = num_iterations.bits();
    for i in (0..nbits - 1).rev() {
        res = nudupl_form(&res, d);
        res.reduce();
        if num_iterations.bit(i) {
            res = nucomp_form(&res, x, d, l);
        }
        res.reduce();
    }
    res.reduce();
    res
}

/// Form squaring: r = f * f.
fn nudupl_form(f: &Form, d: &BigInt) -> Form {
    let (a1, c1) = (f.a.clone(), f.c.clone());
    let b_copy = f.b.clone();
    let (s, v2) = if b_copy.sign() == Sign::Minus {
        let b_abs = -f.b.clone();
        let (g, _s, v) = extended_gcd(&b_abs, &a1);
        (g, -v)
    } else {
        let (g, _s, v) = extended_gcd(&b_copy, &a1);
        (g, v)
    };
    let mut k: BigInt = (-(&v2 * &c1)) % &a1;
    let (a1, c1) = if s != BigInt::from(1) {
        (&a1 / &s, &c1 * &s)
    } else {
        (a1.clone(), c1.clone())
    };
    k = &k % &a1;
    let l = compute_l(d);
    let (ca, cb, _cc) = if a1 < l {
        let t = &a1 * &k;
        let ca = &a1 * &a1;
        let cb = (t.clone() << 1) + &f.b;
        let cc = (&f.b + &t) * &k + &c1;
        let cc = cc / &a1;
        (ca, cb, cc)
    } else {
        let mut co2 = BigInt::from(0);
        let mut co1 = BigInt::from(0);
        let mut r2 = a1.clone();
        let mut r1 = k;
        xgcd_partial_simple(&mut co2, &mut co1, &mut r2, &mut r1, &l);
        let m2 = (&f.b * &r1 - &c1 * &co1) / &a1;
        let mut ca = &r1 * &r1 - &co1 * &m2;
        if co1.sign() == Sign::Minus {
            ca = -ca;
        }
        let cb = (BigInt::from(2) * (&a1 * &r1 - &ca * &co2)) / &co1 - &f.b;
        let cb = cb % (BigInt::from(2) * &ca);
        let cc = (&cb * &cb - d) / (BigInt::from(4) * &ca);
        if ca.sign() == Sign::Minus {
            (-ca, -cb, -cc)
        } else {
            (ca, cb, cc)
        }
    };
    let mut form = Form::from_abd(ca, cb, d).unwrap();
    form.reduce();
    form
}

/// Form composition: res = f * g.
fn nucomp_form(f: &Form, g: &Form, d: &BigInt, l: &BigInt) -> Form {
    if f.a > g.a {
        return nucomp_form(g, f, d, l);
    }
    let (mut a1, mut a2, mut c2) = (f.a.clone(), g.a.clone(), g.c.clone());
    let ss = (&f.b + &g.b) >> 1;
    let m = (&f.b - &g.b) >> 1;
    let t = &a2 % &a1;
    let (sp, v1) = if t.is_zero() {
        (a1.clone(), BigInt::from(0))
    } else {
        let (g, s, _) = extended_gcd(&t, &a1);
        (g, s)
    };
    let mut k: BigInt = (&m * &v1) % &a1;
    if sp != BigInt::from(1) {
        let (s, v2, u2) = extended_gcd(&ss, &sp);
        k = (&k * &u2 - &v2 * &c2) % &a1;
        if s != BigInt::from(1) {
            a1 = a1 / &s;
            a2 = a2 / &s;
            c2 = &c2 * &s;
        }
    }
    k = &k % &a1;
    let (ca, cb, _cc) = if a1 < *l {
        let t = &a2 * &k;
        let ca = &a2 * &a1;
        let cb = (t.clone() << 1) + &g.b;
        let cc = (&g.b + &t) * &k + &c2;
        let cc = cc / &a1;
        (ca, cb, cc)
    } else {
        let mut co2 = BigInt::from(0);
        let mut co1 = BigInt::from(0);
        let mut a1_copy = a1.clone();
        let mut k_copy = k.clone();
        xgcd_partial_simple(&mut co2, &mut co1, &mut a1_copy, &mut k_copy, l);
        let m1 = (&m * &co1 + &a2 * &k_copy) / &a1;
        let m2 = (&ss * &k_copy - &c2 * &co1) / &a1;
        let mut ca: BigInt = &k_copy * &m1 - &co1 * &m2;
        if co1.sign() == Sign::Minus {
            ca = -ca;
        }
        let cb = (BigInt::from(2) * (&k_copy - &ca * &co2)) / &co1 - &g.b;
        let cb = cb % (BigInt::from(2) * &ca);
        let cc = (&cb * &cb - d) / (BigInt::from(4) * &ca);
        if ca.sign() == Sign::Minus {
            (-ca, -cb, -cc)
        } else {
            (ca, cb, cc)
        }
    };
    let mut form = Form::from_abd(ca, cb, d).unwrap();
    form.reduce();
    form
}

fn bytes_to_u64_be(bytes: &[u8]) -> u64 {
    let mut buf = [0u8; 8];
    let len = bytes.len().min(8);
    buf[8 - len..].copy_from_slice(&bytes[..len]);
    u64::from_be_bytes(buf)
}

/// Verify single Weso segment: check proof^B * x^r == xnew, with r = 2^segment_iters mod B.
/// Returns true if segment is invalid (B != GetB).
fn verify_weso_segment(
    d: &BigInt,
    x: &Form,
    proof: &Form,
    b: &BigInt,
    segment_iters: u64,
    xnew: &mut Form,
) -> bool {
    let l = compute_l(d);
    let r = fast_pow(segment_iters, b);
    let f1 = fast_pow_form(proof, d, b, &l);
    let f2 = fast_pow_form(x, d, &r, &l);
    *xnew = nucomp_form(&f1, &f2, d, &l);
    xnew.reduce();
    let b_check = get_b(d, x, xnew);
    b_check != *b
}

/// Verify final Weso step: B = GetB(D,x,y), proof^B * x^r == y.
fn verify_wesolowski_proof(
    d: &BigInt,
    x: &Form,
    y: &Form,
    proof: &Form,
    iters: u64,
) -> bool {
    let l = compute_l(d);
    let b = get_b(d, x, y);
    let r = fast_pow(iters, &b);
    let f1 = fast_pow_form(proof, d, &b, &l);
    let f2 = fast_pow_form(x, d, &r, &l);
    let mut result = nucomp_form(&f1, &f2, d, &l);
    result.reduce();
    result.a == y.a && result.b == y.b && result.c == y.c
}

/// Main N-Wesolowski verification. discriminant_bytes is the discriminant (e.g. 128 bytes for 1024 bits).
pub fn verify_n_wesolowski(
    discriminant_bytes: &[u8],
    x_s: &[u8],
    proof_blob: &[u8],
    num_iterations: u64,
    recursion: u64,
) -> bool {
    let depth = recursion as i32;
    if depth < 0 {
        return false;
    }
    let form_size = BQFC_FORM_SIZE;
    let segment_len = 8 + B_BYTES + form_size;
    let expected_len = 2 * form_size + depth as usize * segment_len;
    if proof_blob.len() != expected_len || x_s.len() < form_size {
        return false;
    }

    let d_bits = discriminant_bytes.len() * 8;
    let d = BigInt::from_bytes_be(Sign::Minus, discriminant_bytes);
    let d = -d;

    let x = match bqfc_deserialize(&d, &x_s[..form_size], d_bits) {
        Ok(f) => f,
        Err(()) => return false,
    };

    let mut iterations = num_iterations;
    let mut x_current = x;
    let mut i = proof_blob.len() - segment_len;

    while i >= 2 * form_size {
        let segment_iters = bytes_to_u64_be(&proof_blob[i..i + 8]);
        let b = BigInt::from_bytes_be(Sign::Plus, &proof_blob[i + 8..i + 8 + B_BYTES]);
        let proof = match bqfc_deserialize(&d, &proof_blob[i + 8 + B_BYTES..i + segment_len], d_bits) {
            Ok(f) => f,
            Err(()) => return false,
        };
        let mut xnew = Form::identity(&d);
        if verify_weso_segment(&d, &x_current, &proof, &b, segment_iters, &mut xnew) {
            return false;
        }
        x_current = xnew;
        if segment_iters > iterations {
            return false;
        }
        iterations -= segment_iters;
        i -= segment_len;
    }

    let y = match bqfc_deserialize(&d, &proof_blob[..form_size], d_bits) {
        Ok(f) => f,
        Err(()) => return false,
    };
    let proof = match bqfc_deserialize(&d, &proof_blob[form_size..2 * form_size], d_bits) {
        Ok(f) => f,
        Err(()) => return false,
    };
    verify_wesolowski_proof(&d, &x_current, &y, &proof, iterations)
}
