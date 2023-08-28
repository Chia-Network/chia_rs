use clvmr::{allocator::NodePtr, Allocator};
use num_bigint::BigInt;

use crate::{Result, Value};

pub trait ClvmTree<N> {
    fn collect_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N>;
}

pub trait ToClvm {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr>;
}

impl<T> ToClvm for T
where
    T: ClvmTree<NodePtr>,
{
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        self.collect_tree(&mut |value| {
            Ok(match value {
                Value::Atom(atom) => match atom {
                    [] => a.null(),
                    [1] => a.one(),
                    _ => a.new_atom(atom)?,
                },
                Value::Pair(first, rest) => a.new_pair(first, rest)?,
            })
        })
    }
}

impl ClvmTree<NodePtr> for NodePtr {
    fn collect_tree(
        &self,
        _f: &mut impl FnMut(Value<NodePtr>) -> Result<NodePtr>,
    ) -> Result<NodePtr> {
        Ok(*self)
    }
}

macro_rules! clvm_primitive {
    ($primitive:ty) => {
        impl<N> ClvmTree<N> for $primitive {
            fn collect_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
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

impl<N, T> ClvmTree<N> for &T
where
    T: ClvmTree<N>,
{
    fn collect_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        T::collect_tree(*self, f)
    }
}

impl<N, A, B> ClvmTree<N> for (A, B)
where
    A: ClvmTree<N>,
    B: ClvmTree<N>,
{
    fn collect_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        let first = self.0.collect_tree(f)?;
        let rest = self.1.collect_tree(f)?;
        f(Value::Pair(first, rest))
    }
}

impl<N> ClvmTree<N> for () {
    fn collect_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        f(Value::Atom(&[]))
    }
}

impl<N, T> ClvmTree<N> for &[T]
where
    T: ClvmTree<N>,
{
    fn collect_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        let mut result = ().collect_tree(f)?;
        for item in self.iter().rev() {
            let value = item.collect_tree(f)?;
            result = f(Value::Pair(value, result))?;
        }
        Ok(result)
    }
}

impl<N, T, const LEN: usize> ClvmTree<N> for [T; LEN]
where
    T: ClvmTree<N>,
{
    fn collect_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        self.as_slice().collect_tree(f)
    }
}

impl<N, T> ClvmTree<N> for Vec<T>
where
    T: ClvmTree<N>,
{
    fn collect_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        self.as_slice().collect_tree(f)
    }
}

impl<N, T> ClvmTree<N> for Option<T>
where
    T: ClvmTree<N>,
{
    fn collect_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        match self {
            Some(value) => value.collect_tree(f),
            None => ().collect_tree(f),
        }
    }
}

impl<N> ClvmTree<N> for &str {
    fn collect_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        f(Value::Atom(self.as_bytes()))
    }
}

impl<N> ClvmTree<N> for String {
    fn collect_tree(&self, f: &mut impl FnMut(Value<N>) -> Result<N>) -> Result<N> {
        self.as_str().collect_tree(f)
    }
}

#[cfg(test)]
mod tests {
    use clvmr::{serde::node_to_bytes, Allocator};
    use hex::ToHex;

    use super::*;

    fn encode<T>(a: &mut Allocator, value: T) -> Result<String>
    where
        T: ClvmTree<NodePtr>,
    {
        let actual = value.to_clvm(a).unwrap();
        let actual_bytes = node_to_bytes(a, actual).unwrap();
        Ok(actual_bytes.encode_hex())
    }

    fn check<T>(a: &mut Allocator, value: T, hex: &str)
    where
        T: ClvmTree<NodePtr>,
    {
        let result = encode(a, value).unwrap();
        assert_eq!(result, hex);
    }

    #[test]
    fn test_nodeptr() {
        let a = &mut Allocator::new();
        let ptr = a.one();
        assert_eq!(ptr.to_clvm(a).unwrap(), ptr);
    }

    #[test]
    fn test_primitives() {
        let a = &mut Allocator::new();
        check(a, 0u8, "80");
        check(a, 0i8, "80");
        check(a, 5u8, "05");
        check(a, 5u32, "05");
        check(a, 5i32, "05");
        check(a, -27i32, "81e5");
        check(a, -0, "80");
        check(a, -128i8, "8180");
    }

    #[test]
    fn test_reference() {
        let a = &mut Allocator::new();
        check(a, &[1, 2, 3], "ff01ff02ff0380");
        check(a, &Some(42), "2a");
        check(a, &Some(&42), "2a");
        check(a, Some(&42), "2a");
    }

    #[test]
    fn test_pair() {
        let a = &mut Allocator::new();
        check(a, (5, 2), "ff0502");
        check(a, (-72, (90121, ())), "ff81b8ff8301600980");
        check(
            a,
            (((), ((), ((), (((), ((), ((), ()))), ())))), ()),
            "ffff80ff80ff80ffff80ff80ff80808080",
        );
    }

    #[test]
    fn test_nil() {
        let a = &mut Allocator::new();
        check(a, (), "80");
    }

    #[test]
    fn test_slice() {
        let a = &mut Allocator::new();
        check(a, [1, 2, 3, 4].as_slice(), "ff01ff02ff03ff0480");
        check(a, [0; 0].as_slice(), "80");
    }

    #[test]
    fn test_array() {
        let a = &mut Allocator::new();
        check(a, [1, 2, 3, 4], "ff01ff02ff03ff0480");
        check(a, [0; 0], "80");
    }

    #[test]
    fn test_vec() {
        let a = &mut Allocator::new();
        check(a, vec![1, 2, 3, 4], "ff01ff02ff03ff0480");
        check(a, vec![0; 0], "80");
    }

    #[test]
    fn test_option() {
        let a = &mut Allocator::new();
        check(a, Some("hello"), "8568656c6c6f");
        check(a, None::<&str>, "80");
        check(a, Some(""), "80");
    }

    #[test]
    fn test_str() {
        let a = &mut Allocator::new();
        check(a, "hello", "8568656c6c6f");
        check(a, "", "80");
    }

    #[test]
    fn test_string() {
        let a = &mut Allocator::new();
        check(a, "hello".to_string(), "8568656c6c6f");
        check(a, "".to_string(), "80");
    }
}
