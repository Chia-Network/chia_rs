//! Quadratic form (a, b, c) with discriminant D, where b^2 - 4ac = D.
//! Always in reduced form: |b| <= a <= c, with b >= 0 when a == c or |b| == a.

use crate::integer::num_bits;
use crate::reducer::reduce;
use malachite_base::num::arithmetic::traits::DivRem;
use malachite_base::num::basic::traits::One;
use malachite_nz::integer::Integer;

/// A binary quadratic form with coefficients (a, b, c) and discriminant D.
#[derive(Clone, Debug)]
pub struct Form {
    pub a: Integer,
    pub b: Integer,
    pub c: Integer,
}

impl Form {
    pub fn new(a: Integer, b: Integer, c: Integer) -> Self {
        Form { a, b, c }
    }

    /// Construct form from (a, b) and discriminant D, computing c = (b^2 - D) / (4a).
    /// Returns Err if a <= 0 or (b^2 - D) is not divisible by 4a.
    pub fn from_abd(a: Integer, b: Integer, d: &Integer) -> Result<Self, String> {
        if a <= 0i32 {
            return Err("Invalid form: a must be positive".into());
        }
        let b2 = &b * &b;
        let num = b2 - d;
        let denom = &a << 2u64;
        let (c, rem) = (&num).div_rem(&denom);
        if rem != 0i32 {
            return Err("Invalid form: can't find c".into());
        }
        Ok(Form { a, b, c })
    }

    /// Identity form: (1, 1, (1-D)/4).
    pub fn identity(d: &Integer) -> Self {
        let a = Integer::ONE;
        let b = Integer::ONE;
        let num = Integer::ONE - d;
        let c = num >> 2u64;
        Form { a, b, c }
    }

    /// Generator form: (2, 1, (1-D)/8) — only valid when D ≡ 1 mod 8.
    pub fn generator(d: &Integer) -> Self {
        let a = Integer::from(2i32);
        let b = Integer::ONE;
        let num = Integer::ONE - d;
        let c = num >> 3u64;
        Form { a, b, c }
    }

    /// Check if this is the identity form (a=1, b=1).
    pub fn is_identity(&self) -> bool {
        self.a == 1i32 && self.b == 1i32
    }

    /// Check if this is the generator form (a=2, b=1).
    pub fn is_generator(&self) -> bool {
        self.a == 2i32 && self.b == 1i32
    }

    /// Check if this form is reduced: |b| <= a <= c, with b >= 0 when a == c or |b| == a.
    pub fn is_reduced(&self) -> bool {
        let abs_b = self.b.unsigned_abs_ref();
        let abs_a = self.a.unsigned_abs_ref();
        let abs_c = self.c.unsigned_abs_ref();
        if abs_b > abs_a {
            return false;
        }
        if abs_a > abs_c {
            return false;
        }
        if self.a == self.c && self.b < 0i32 {
            return false;
        }
        if abs_b == abs_a && self.b < 0i32 {
            return false;
        }
        true
    }

    /// Reduce this form in place using the Pulmark reducer.
    pub fn reduce(&mut self) {
        reduce(self);
    }

    /// The half-max size parameter L = floor((-D)^(1/4)).
    /// Used as a threshold in nucomp.
    pub fn compute_l(d: &Integer) -> Integer {
        let neg_d = d.unsigned_abs_ref().clone();
        crate::integer::nth_root(&Integer::from(neg_d), 4)
    }

    /// Discriminant size in bits.
    pub fn d_bits(d: &Integer) -> usize {
        num_bits(d)
    }
}

impl PartialEq for Form {
    fn eq(&self, other: &Self) -> bool {
        self.a == other.a && self.b == other.b && self.c == other.c
    }
}

impl Eq for Form {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_form() {
        let d = Integer::from(-47i64);
        let f = Form::identity(&d);
        let disc = &f.b * &f.b - Integer::from(4i32) * &f.a * &f.c;
        assert_eq!(disc, d, "identity form discriminant check");
    }
}
