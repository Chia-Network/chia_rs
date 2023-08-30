use clvmr::allocator::NodePtr;
use num_bigint::BigInt;

use crate::{Result, Value};

pub trait BuildTree<N> {
    fn build_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N>;
}

impl BuildTree<NodePtr> for NodePtr {
    fn build_tree(
        &self,
        _f: &mut impl FnMut(Value<NodePtr>) -> Result<NodePtr>,
    ) -> Result<NodePtr> {
        Ok(*self)
    }
}

macro_rules! clvm_primitive {
    ($primitive:ty) => {
        impl<N> BuildTree<N> for $primitive {
            fn build_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
                if *self == 0 {
                    f(Value::Atom(&[]))
                } else {
                    let value: BigInt = (*self).into();
                    f(Value::Atom(&value.to_signed_bytes_be()))
                }
            }
        }
    };
}

clvm_primitive!(u8);
clvm_primitive!(i8);
clvm_primitive!(u16);
clvm_primitive!(i16);
clvm_primitive!(u32);
clvm_primitive!(i32);
clvm_primitive!(u64);
clvm_primitive!(i64);
clvm_primitive!(u128);
clvm_primitive!(i128);
clvm_primitive!(usize);
clvm_primitive!(isize);

impl<N, T> BuildTree<N> for &T
where
    T: BuildTree<N>,
{
    fn build_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        T::build_tree(*self, f)
    }
}

impl<N, A, B> BuildTree<N> for (A, B)
where
    A: BuildTree<N>,
    B: BuildTree<N>,
{
    fn build_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        let first = self.0.build_tree(f)?;
        let rest = self.1.build_tree(f)?;
        f(Value::Pair(first, rest))
    }
}

impl<N> BuildTree<N> for () {
    fn build_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        f(Value::Atom(&[]))
    }
}

impl<N, T> BuildTree<N> for &[T]
where
    T: BuildTree<N>,
{
    fn build_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        let mut result = ().build_tree(f)?;
        for item in self.iter().rev() {
            let value = item.build_tree(f)?;
            result = f(Value::Pair(value, result))?;
        }
        Ok(result)
    }
}

impl<N, T, const LEN: usize> BuildTree<N> for [T; LEN]
where
    T: BuildTree<N>,
{
    fn build_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        self.as_slice().build_tree(f)
    }
}

impl<N, T> BuildTree<N> for Vec<T>
where
    T: BuildTree<N>,
{
    fn build_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        self.as_slice().build_tree(f)
    }
}

impl<N, T> BuildTree<N> for Option<T>
where
    T: BuildTree<N>,
{
    fn build_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        match self {
            Some(value) => value.build_tree(f),
            None => ().build_tree(f),
        }
    }
}

impl<N> BuildTree<N> for &str {
    fn build_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        f(Value::Atom(self.as_bytes()))
    }
}

impl<N> BuildTree<N> for String {
    fn build_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        self.as_str().build_tree(f)
    }
}
