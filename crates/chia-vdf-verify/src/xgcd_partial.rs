//! Partial extended GCD (Lehmer-accelerated).
//!
//! Port of `mpz_xgcd_partial` from chiavdf/src/xgcd_partial.c.
//!
//! On exit: co2*r1_orig - co1*r2_orig = ±r2_final
//! Terminates when r1 <= L.

use crate::integer::{bitlen_nonneg, extract_uword_from_shift_nonneg, LIMB_BITS};
use malachite_base::num::arithmetic::traits::DivRem;
use malachite_base::num::basic::traits::{NegativeOne, Zero};
use malachite_nz::integer::Integer;

/// Partial extended GCD.
/// Inputs: r2, r1, L (all non-negative, r2 >= r1).
/// On return:
///   - r1 <= L (termination condition)
///   - co2*r1_in - co1*r2_in = ±r2_out  (approximately)
///   - r2_out >= 0
pub fn xgcd_partial(
    co2: &mut Integer,
    co1: &mut Integer,
    r2: &mut Integer,
    r1: &mut Integer,
    l: &Integer,
) {
    *co2 = Integer::ZERO;
    *co1 = Integer::NEGATIVE_ONE;

    while *r1 != 0i32 && &*r1 > l {
        let bits2 = bitlen_nonneg(r2);
        let bits1 = bitlen_nonneg(r1);
        let bits = std::cmp::max(bits2, bits1) - LIMB_BITS as i64 + 1;
        let bits = if bits < 0 { 0 } else { bits };

        let mut rr2 = extract_uword_from_shift_nonneg(r2, bits);
        let mut rr1 = extract_uword_from_shift_nonneg(r1, bits);
        let bb = extract_uword_from_shift_nonneg(l, bits);

        let mut aa2: i64 = 0;
        let mut aa1: i64 = 1;
        let mut bb2: i64 = 1;
        let mut bb1: i64 = 0;

        let mut i = 0usize;
        loop {
            if rr1 == 0 || rr1 <= bb {
                break;
            }
            let qq = rr2 / rr1;

            let t1 = rr2 - qq * rr1;
            let t2 = aa2 - qq * aa1;
            let t3 = bb2 - qq * bb1;

            if i & 1 != 0 {
                if t1 < -t3 || rr1 - t1 < t2 - aa1 {
                    break;
                }
            } else if t1 < -t2 || rr1 - t1 < t3 - bb1 {
                break;
            }

            rr2 = rr1;
            rr1 = t1;
            aa2 = aa1;
            aa1 = t2;
            bb2 = bb1;
            bb1 = t3;
            i += 1;
        }

        if i == 0 {
            let (q, rem) = (&*r2).div_rem(&*r1);
            *r2 = std::mem::replace(r1, rem);
            *co2 -= &q * &*co1;
            std::mem::swap(co2, co1);
        } else {
            let r = r2.clone() * Integer::from(bb2) + r1.clone() * Integer::from(aa2);
            let new_r1 = r1.clone() * Integer::from(aa1) + r2.clone() * Integer::from(bb1);
            *r2 = r;
            *r1 = new_r1;

            let new_co2 = co2.clone() * Integer::from(bb2) + co1.clone() * Integer::from(aa2);
            let new_co1 = co1.clone() * Integer::from(aa1) + co2.clone() * Integer::from(bb1);
            *co2 = new_co2;
            *co1 = new_co1;

            if *r1 < 0i32 {
                *co1 = -co1.clone();
                *r1 = -r1.clone();
            }
            if *r2 < 0i32 {
                *co2 = -co2.clone();
                *r2 = -r2.clone();
            }
        }
    }

    if *r2 < 0i32 {
        *co2 = -co2.clone();
        *co1 = -co1.clone();
        *r2 = -r2.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xgcd_partial_basic() {
        let mut co2 = Integer::ZERO;
        let mut co1 = Integer::ZERO;
        let mut r2 = Integer::from(100u32);
        let mut r1 = Integer::from(37u32);
        let l = Integer::from(10u32);

        xgcd_partial(&mut co2, &mut co1, &mut r2, &mut r1, &l);

        assert!(r1 <= l, "r1={} should be <= L={}", r1, l);
        assert!(r2 >= 0i32, "r2 should be non-negative");
    }

    #[test]
    fn test_xgcd_partial_identity() {
        let mut co2 = Integer::ZERO;
        let mut co1 = Integer::ZERO;
        let mut r2 = Integer::from(100u32);
        let mut r1 = Integer::from(5u32);
        let l = Integer::from(10u32);

        xgcd_partial(&mut co2, &mut co1, &mut r2, &mut r1, &l);

        assert!(r1 <= l);
    }
}
