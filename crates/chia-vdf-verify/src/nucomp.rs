//! NUCOMP and NUDUPL form composition.
//!
//! Port of chiavdf/src/nucomp.h (William Hart's algorithm).

use crate::form::Form;
use crate::integer::{divexact, fast_extended_gcd, fast_gcd_coeff_b, fdiv_q, fdiv_r, tdiv_r};
use crate::xgcd_partial::xgcd_partial;
use malachite_base::num::basic::traits::Zero;
use malachite_nz::integer::Integer;

/// Compose two forms: result = f * g.
/// This is qfb_nucomp.
pub fn nucomp(f: &Form, g: &Form, d: &Integer, l: &Integer) -> Form {
    if f.a > g.a {
        return nucomp(g, f, d, l);
    }

    let a1 = f.a.clone();
    let a2 = g.a.clone();
    let c2 = g.c.clone();

    let ss = (&f.b + &g.b) >> 1u64;
    let m = (&f.b - &g.b) >> 1u64;

    let t = fdiv_r(&a2, &a1);
    let (sp, v1) = if t == 0i32 {
        (a1.clone(), Integer::ZERO)
    } else {
        let (gcd, x, _) = fast_extended_gcd(&t, &a1);
        (gcd, x)
    };

    let mut k = fdiv_r(&(&m * &v1), &a1);

    let (a1_new, a2_new, c2_new);

    if sp != 1i32 {
        let (s, v2, u2) = gcd_ext3(&ss, &sp);

        k = &k * &u2 - &v2 * &c2;

        if s != 1i32 {
            a1_new = divexact(&a1, &s);
            a2_new = divexact(&a2, &s);
            c2_new = &c2 * &s;
        } else {
            a1_new = a1.clone();
            a2_new = a2.clone();
            c2_new = c2.clone();
        }

        k = fdiv_r(&k, &a1_new);
    } else {
        a1_new = a1.clone();
        a2_new = a2.clone();
        c2_new = c2.clone();
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

        let m1 = divexact(&(&m * &co1 + &a2_new * &r1), &a1_new);

        let m2 = divexact(&(&ss * &r1 - &c2_new * &co1), &a1_new);

        let ca_unsigned = &r1 * &m1 - &co1 * &m2;
        let mut ca = if co1 < 0i32 {
            ca_unsigned
        } else {
            -ca_unsigned
        };

        let t_val = &a2_new * &r1;

        let cb_inner = &t_val - &ca * &co2;
        let cb_scaled = &cb_inner << 1u64;
        let cb_divided = divexact(&cb_scaled, &co1);
        let cb_shifted = cb_divided - &g.b;
        let ca2 = &ca << 1u64;
        let cb = fdiv_r(&cb_shifted, &ca2);

        let cc_num = &cb * &cb - d;
        let cc_denom = &ca << 2u64;
        let mut cc = divexact(&cc_num, &cc_denom);

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
    let a1 = f.a.clone();
    let c1 = f.c.clone();

    let b_abs = if f.b < 0i32 {
        -f.b.clone()
    } else {
        f.b.clone()
    };
    let (s, v2) = {
        let (gcd, coeff_b) = fast_gcd_coeff_b(&a1, &b_abs);
        let v2 = if f.b < 0i32 { -coeff_b } else { coeff_b };
        (gcd, v2)
    };

    let k_raw = -&c1 * &v2;
    let mut k = tdiv_r(&k_raw, &a1);
    if k < 0i32 {
        k += &a1;
    }

    let a1_new;
    let c1_new;

    let s_is_1 = s == 1i32;
    if !s_is_1 {
        a1_new = fdiv_q(&a1, &s);
        c1_new = &c1 * &s;
    } else {
        a1_new = a1.clone();
        c1_new = c1.clone();
    }

    if a1_new < *l {
        let t = &a1_new * &k;
        let new_a = &a1_new * &a1_new;
        let cb = (&t << 1u64) + &f.b;
        let cc_num = (&f.b + &t) * &k + &c1_new;
        let cc = fdiv_q(&cc_num, &a1_new);

        Form::new(new_a, cb, cc)
    } else {
        let mut r2 = a1_new.clone();
        let mut r1 = k;
        let mut co2 = Integer::ZERO;
        let mut co1 = Integer::ZERO;

        xgcd_partial(&mut co2, &mut co1, &mut r2, &mut r1, l);

        let m2_num = &f.b * &r1 - &c1_new * &co1;
        let m2 = divexact(&m2_num, &a1_new);

        let mut new_a = &r1 * &r1 - &co1 * &m2;
        if co1 >= 0i32 {
            new_a = -new_a;
        }

        let cb_tmp = &new_a * &co2 - &a1_new * &r1;
        let cb_neg = -cb_tmp;
        let cb_doubled = &cb_neg << 1u64;
        let cb_div = divexact(&cb_doubled, &co1);
        let cb_pre = cb_div - &f.b;
        let two_new_a = &new_a << 1u64;
        let cb = fdiv_r(&cb_pre, &two_new_a);

        let cc_num = &cb * &cb - d;
        let cc_pre = divexact(&cc_num, &new_a);
        let cc = &cc_pre >> 2u64;

        let (final_a, final_c) = if new_a < 0i32 {
            (-new_a, -cc)
        } else {
            (new_a, cc)
        };

        Form::new(final_a, cb, final_c)
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
