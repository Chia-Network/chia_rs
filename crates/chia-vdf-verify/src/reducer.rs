//! Pulmark form reducer.
//!
//! Port of chiavdf/src/Reducer.h.
//! Reduces a quadratic form (a, b, c) to its canonical reduced representative.

use crate::form::Form;
use crate::integer::get_si_2exp;
use malachite_base::num::arithmetic::traits::CeilingDivMod;
use malachite_nz::integer::Integer;
use malachite_nz::natural::Natural;

const THRESH: i64 = 1i64 << 31;
const EXP_THRESH: i64 = 31;

/// Reduce form f in place.
pub fn reduce(f: &mut Form) {
    while !is_reduced(f) {
        let (a_val, a_exp) = get_si_2exp(&f.a);
        let (b_val, b_exp) = get_si_2exp(&f.b);
        let (c_val, c_exp) = get_si_2exp(&f.c);

        let max_exp = *[a_exp, b_exp, c_exp].iter().max().unwrap() + 1;
        let min_exp = *[a_exp, b_exp, c_exp].iter().min().unwrap();

        if max_exp - min_exp > EXP_THRESH {
            reducer_simple(f);
            continue;
        }

        let a_sh = max_exp - a_exp;
        let b_sh = max_exp - b_exp;
        let c_sh = max_exp - c_exp;

        let a = a_val >> a_sh;
        let b = b_val >> b_sh;
        let c = c_val >> c_sh;

        let (u, v, w, x) = calc_uvwx(a, b, c);

        let new_a = f.a.clone() * Integer::from(u * u)
            + f.b.clone() * Integer::from(u * w)
            + f.c.clone() * Integer::from(w * w);
        let new_b = f.a.clone() * Integer::from(2 * u * v)
            + f.b.clone() * Integer::from(u * x + v * w)
            + f.c.clone() * Integer::from(2 * w * x);
        let new_c = f.a.clone() * Integer::from(v * v)
            + f.b.clone() * Integer::from(v * x)
            + f.c.clone() * Integer::from(x * x);

        f.a = new_a;
        f.b = new_b;
        f.c = new_c;
    }
}

/// Simple reducer step (used when exponent spread is large).
fn reducer_simple(f: &mut Form) {
    let r = ceildiv(&f.b, &f.c);
    let r_plus1 = r + Integer::from(1i32);
    let s = r_plus1 >> 1u64;

    let cs = &f.c * &s;
    let cs2 = &cs << 1u64;

    let m = &cs - &f.b;

    let new_b = &cs2 - &f.b;

    let old_a = f.a.clone();

    f.a = f.c.clone();

    f.c = old_a + &s * &m;
    f.b = new_b;
}

/// Ceiling division: ceil(a/b).
fn ceildiv(a: &Integer, b: &Integer) -> Integer {
    if *b > 0i32 {
        a.ceiling_div_mod(b).0
    } else {
        a / b
    }
}

/// Check if the form is reduced and normalize if needed.
/// Returns true if already reduced (but may have swapped a/c or negated b).
fn is_reduced(f: &mut Form) -> bool {
    let abs_a: Natural = f.a.unsigned_abs_ref().clone();
    let abs_b: Natural = f.b.unsigned_abs_ref().clone();
    let abs_c: Natural = f.c.unsigned_abs_ref().clone();

    if abs_a < abs_b || abs_c < abs_b {
        return false;
    }

    let a_cmp_c = abs_a.cmp(&abs_c);
    if a_cmp_c == std::cmp::Ordering::Greater {
        std::mem::swap(&mut f.a, &mut f.c);
        f.b = -f.b.clone();
    } else if a_cmp_c == std::cmp::Ordering::Equal && f.b < 0i32 {
        f.b = -f.b.clone();
    }
    true
}

/// Lehmer acceleration step: compute (u, v, w, x) 2x2 matrix.
fn calc_uvwx(mut a: i64, mut b: i64, mut c: i64) -> (i64, i64, i64, i64) {
    let mut u_ = 1i64;
    let mut v_ = 0i64;
    let mut w_ = 0i64;
    let mut x_ = 1i64;

    let mut u;
    let mut v;
    let mut w;
    let mut x;

    loop {
        u = u_;
        v = v_;
        w = w_;
        x = x_;

        if c == 0 {
            break;
        }

        let s = if b >= 0 {
            (b + c) / (c << 1)
        } else {
            -(-b + c) / (c << 1)
        };

        let a_ = a;
        let b_ = b;

        a = c;
        b = -b + (c.wrapping_mul(s) << 1);
        c = a_ - s * (b_ - c.wrapping_mul(s));

        u_ = v;
        v_ = -u + s.wrapping_mul(v);
        w_ = x;
        x_ = -w + s.wrapping_mul(x);

        let below_threshold = (v_.abs() | x_.abs()) <= THRESH;
        if !(below_threshold && a > c && c > 0) {
            if below_threshold {
                u = u_;
                v = v_;
                w = w_;
                x = x_;
            }
            break;
        }
    }

    (u, v, w, x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::Form;

    fn disc_check(f: &Form, d: &Integer) -> bool {
        let disc = &f.b * &f.b - Integer::from(4i32) * &f.a * &f.c;
        &disc == d
    }

    #[test]
    fn test_reduce_preserves_discriminant() {
        let d = Integer::from(-47i64);
        let mut f = Form::new(
            Integer::from(3i32),
            Integer::from(1i32),
            Integer::from(4i32),
        );
        assert!(disc_check(&f, &d));
        reduce(&mut f);
        assert!(disc_check(&f, &d), "discriminant changed after reduction");
        assert!(
            f.is_reduced(),
            "form not reduced: a={}, b={}, c={}",
            f.a,
            f.b,
            f.c
        );
    }

    #[test]
    fn test_reduce_idempotent() {
        let d = Integer::from(-47i64);
        let mut f = Form::new(
            Integer::from(2i32),
            Integer::from(1i32),
            Integer::from(6i32),
        );
        assert!(disc_check(&f, &d));
        reduce(&mut f);
        let f2 = f.clone();
        reduce(&mut f);
        assert_eq!(f, f2, "reduction should be idempotent");
    }
}
