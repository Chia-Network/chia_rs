//! NUCOMP and NUDUPL form composition.
//!
//! Port of chiavdf/src/nucomp.h (William Hart's algorithm).

use crate::form::Form;
use crate::integer::{divexact, fast_extended_gcd, fast_gcd_coeff_a_owned, fdiv_q, fdiv_r, tdiv_r};
use crate::xgcd_partial::xgcd_partial;
use malachite_base::num::arithmetic::traits::{AddMulAssign, NegAssign, SubMulAssign};
use malachite_base::num::basic::traits::Zero;
use malachite_nz::integer::Integer;

/// Compose two forms: result = f * g.
/// This is qfb_nucomp.
pub fn nucomp(f: &Form, g: &Form, d: &Integer, l: &Integer) -> Form {
    if f.a > g.a {
        return nucomp(g, f, d, l);
    }

    let ss = (&f.b + &g.b) >> 1u64;
    let m = (&f.b - &g.b) >> 1u64;

    let t = fdiv_r(&g.a, &f.a);
    let (sp, v1) = if t == 0i32 {
        (f.a.clone(), Integer::ZERO)
    } else {
        let (gcd, x) = fast_gcd_coeff_a_owned(t, f.a.clone());
        (gcd, x)
    };

    let mut k = fdiv_r(&(&m * &v1), &f.a);

    let (a1_new, a2_new, c2_new);

    if sp == 1i32 {
        a1_new = f.a.clone();
        a2_new = g.a.clone();
        c2_new = g.c.clone();
    } else {
        let (s, v2, u2) = gcd_ext3(&ss, &sp);

        k *= &u2;
        k.sub_mul_assign(&v2, &g.c);

        if s == 1i32 {
            a1_new = f.a.clone();
            a2_new = g.a.clone();
            c2_new = g.c.clone();
        } else {
            a1_new = divexact(&f.a, &s);
            a2_new = divexact(&g.a, &s);
            c2_new = &g.c * &s;
        }

        k = fdiv_r(&k, &a1_new);
    }

    if a1_new < *l {
        let t = &a2_new * &k;
        let ca = &a2_new * &a1_new;
        let cb = (&t << 1u64) + &g.b;
        let cc_num = (&g.b + &t) * &k + &c2_new;
        let cc = divexact(&cc_num, &a1_new);

        Form::new(ca, cb, cc)
    } else {
        let mut r2 = a1_new.clone();
        let mut r1 = k;
        let mut co2 = Integer::ZERO;
        let mut co1 = Integer::ZERO;

        xgcd_partial(&mut co2, &mut co1, &mut r2, &mut r1, l);

        let mut m1_num = &m * &co1;
        m1_num.add_mul_assign(&a2_new, &r1);
        let m1 = divexact(&m1_num, &a1_new);

        let mut m2_num = &ss * &r1;
        m2_num.sub_mul_assign(&c2_new, &co1);
        let m2 = divexact(&m2_num, &a1_new);

        let mut ca_unsigned = &r1 * &m1;
        ca_unsigned.sub_mul_assign(&co1, &m2);
        let mut ca = if co1 < 0i32 {
            ca_unsigned
        } else {
            -ca_unsigned
        };

        let t_val = &a2_new * &r1;

        let mut cb_inner = t_val;
        cb_inner.sub_mul_assign(&ca, &co2);
        let cb_scaled = &cb_inner << 1u64;
        let cb_divided = divexact(&cb_scaled, &co1);
        let cb_shifted = cb_divided - &g.b;
        let ca2 = &ca << 1u64;
        let cb = fdiv_r(&cb_shifted, &ca2);

        let cc_num = &cb * &cb - d;
        let cc_pre = divexact(&cc_num, &ca);
        let mut cc = &cc_pre >> 2u64;

        if ca < 0i32 {
            ca = -ca;
            cc = -cc;
        }

        Form::new(ca, cb, cc)
    }
}

/// Extended GCD returning (gcd, coeff_a, coeff_b) where gcd = coeff_a * a + coeff_b * b.
fn gcd_ext3(a: &Integer, b: &Integer) -> (Integer, Integer, Integer) {
    fast_extended_gcd(a, b)
}

/// Duplicate a form: result = f^2.
/// This is qfb_nudupl.
pub fn nudupl(f: &Form, d: &Integer, l: &Integer) -> Form {
    let b_is_neg = f.b < 0i32;
    let b_abs = if b_is_neg { -&f.b } else { f.b.clone() };
    let (s, v2) = {
        // Swap argument order: pass b_abs first so its cofactor is the
        // native one computed by the GCD algorithm (avoiding the expensive
        // second-cofactor derivation via multiply+divide).
        let (gcd, coeff_b) = fast_gcd_coeff_a_owned(b_abs, f.a.clone());
        let v2 = if b_is_neg { -coeff_b } else { coeff_b };
        (gcd, v2)
    };

    let k_raw = -&f.c * &v2;

    let s_is_1 = s == 1i32;

    if s_is_1 {
        let mut k = tdiv_r(&k_raw, &f.a);
        if k < 0i32 {
            k += &f.a;
        }
        nudupl_inner(&f.a, &f.c, &f.b, d, l, k)
    } else {
        let a1_new = fdiv_q(&f.a, &s);
        let c1_new = &f.c * &s;
        let mut k = tdiv_r(&k_raw, &a1_new);
        if k < 0i32 {
            k += &a1_new;
        }
        nudupl_inner(&a1_new, &c1_new, &f.b, d, l, k)
    }
}

fn nudupl_inner(
    a1: &Integer,
    c1: &Integer,
    fb: &Integer,
    d: &Integer,
    l: &Integer,
    k: Integer,
) -> Form {
    if *a1 < *l {
        let t = a1 * &k;
        let new_a = a1 * a1;
        let cb = (&t << 1u64) + fb;
        let mut cc_num = (fb + &t) * &k;
        cc_num += c1;
        let cc = fdiv_q(&cc_num, a1);
        Form::new(new_a, cb, cc)
    } else {
        let mut r2 = a1.clone();
        let mut r1 = k;
        let mut co2 = Integer::ZERO;
        let mut co1 = Integer::ZERO;

        xgcd_partial(&mut co2, &mut co1, &mut r2, &mut r1, l);

        let mut m2_num = fb * &r1;
        m2_num.sub_mul_assign(c1, &co1);
        let m2 = divexact(&m2_num, a1);

        let mut new_a = &r1 * &r1;
        new_a.sub_mul_assign(&co1, &m2);
        if co1 >= 0i32 {
            new_a.neg_assign();
        }

        let mut cb_tmp = &new_a * &co2;
        cb_tmp.sub_mul_assign(a1, &r1);
        let cb_doubled = (-cb_tmp) << 1u64;
        let cb_div = divexact(&cb_doubled, &co1);
        let cb_pre = cb_div - fb;
        let two_new_a = &new_a << 1u64;
        let cb = fdiv_r(&cb_pre, &two_new_a);

        let cc_num = &cb * &cb - d;
        let cc_pre = divexact(&cc_num, &new_a);
        let cc = &cc_pre >> 2u64;

        if new_a < 0i32 {
            new_a.neg_assign();
            Form::new(new_a, cb, -cc)
        } else {
            Form::new(new_a, cb, cc)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::Form;

    fn discriminant_ok(f: &Form, d: &Integer) -> bool {
        let disc = &f.b * &f.b - Integer::from(4i32) * &f.a * &f.c;
        &disc == d
    }

    #[test]
    fn test_nucomp_preserves_discriminant() {
        let d = Integer::from(-47i64);
        let l = Form::compute_l(&d);
        let f = Form::new(
            Integer::from(2i32),
            Integer::from(1i32),
            Integer::from(6i32),
        );
        let g = Form::new(
            Integer::from(3i32),
            Integer::from(1i32),
            Integer::from(4i32),
        );
        assert!(discriminant_ok(&f, &d));
        assert!(discriminant_ok(&g, &d));
        let result = nucomp(&f, &g, &d, &l);
        assert!(
            discriminant_ok(&result, &d),
            "nucomp result has wrong discriminant: a={}, b={}, c={}",
            result.a,
            result.b,
            result.c
        );
    }

    #[test]
    fn test_nudupl_preserves_discriminant() {
        let d = Integer::from(-47i64);
        let l = Form::compute_l(&d);
        let f = Form::new(
            Integer::from(2i32),
            Integer::from(1i32),
            Integer::from(6i32),
        );
        assert!(discriminant_ok(&f, &d));
        let result = nudupl(&f, &d, &l);
        assert!(
            discriminant_ok(&result, &d),
            "nudupl result has wrong discriminant: a={}, b={}, c={}",
            result.a,
            result.b,
            result.c
        );
    }
}
