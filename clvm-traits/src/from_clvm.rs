use anyhow::Result;
use clvmr::{
    allocator::{NodePtr, SExp},
    Allocator,
};

use crate::{ParseTree, Value};

pub trait FromClvm: ParseTree<NodePtr> {
    fn from_clvm(a: &Allocator, ptr: NodePtr) -> Result<Self>;
}

impl<T> FromClvm for T
where
    T: ParseTree<NodePtr>,
{
    fn from_clvm(a: &Allocator, ptr: NodePtr) -> Result<Self> {
        T::parse_tree(
            &|ptr| match a.sexp(ptr) {
                SExp::Atom => Value::Atom(a.atom(ptr)),
                SExp::Pair(first, rest) => Value::Pair(first, rest),
            },
            ptr,
        )
    }
}

#[cfg(test)]
mod tests {
    use clvmr::serde::node_from_bytes;

    use super::*;

    fn decode<T>(a: &mut Allocator, hex: &str) -> Result<T>
    where
        T: FromClvm,
    {
        let bytes = hex::decode(hex).unwrap();
        let actual = node_from_bytes(a, &bytes).unwrap();
        T::from_clvm(a, actual)
    }

    fn check<T>(a: &mut Allocator, hex: &str, value: T)
    where
        T: FromClvm + PartialEq + std::fmt::Debug,
    {
        let result = decode::<T>(a, hex).unwrap();
        assert_eq!(result, value);
    }

    #[test]
    fn test_nodeptr() {
        let a = &mut Allocator::new();
        let ptr = a.one();
        assert_eq!(NodePtr::from_clvm(a, ptr).unwrap(), ptr);
    }

    #[test]
    fn test_primitives() {
        let a = &mut Allocator::new();
        check(a, "80", 0u8);
        check(a, "80", 0i8);
        check(a, "05", 5u8);
        check(a, "05", 5u32);
        check(a, "05", 5i32);
        check(a, "81e5", -27i32);
        check(a, "80", -0i32);
        check(a, "8180", -128i8);
    }

    #[test]
    fn test_pair() {
        let a = &mut Allocator::new();
        check(a, "ff0502", (5, 2));
        check(a, "ff81b8ff8301600980", (-72, (90121, ())));
        check(
            a,
            "ffff80ff80ff80ffff80ff80ff80808080",
            (((), ((), ((), (((), ((), ((), ()))), ())))), ()),
        );
    }

    #[test]
    fn test_nil() {
        let a = &mut Allocator::new();
        check(a, "80", ());
    }

    #[test]
    fn test_array() {
        let a = &mut Allocator::new();
        check(a, "ff01ff02ff03ff0480", [1, 2, 3, 4]);
        check::<[i32; 0]>(a, "80", []);
    }

    #[test]
    fn test_vec() {
        let a = &mut Allocator::new();
        check(a, "ff01ff02ff03ff0480", vec![1, 2, 3, 4]);
        check(a, "80", Vec::<i32>::new());
    }

    #[test]
    fn test_option() {
        let a = &mut Allocator::new();
        check(a, "8568656c6c6f", Some("hello".to_string()));

        // Empty strings get decoded as None instead, since both values are represented by nil bytes.
        // This could be considered either intended behavior or not, depending on the way it's used.
        check(a, "80", None::<String>);
    }

    #[test]
    fn test_string() {
        let a = &mut Allocator::new();
        check(a, "8568656c6c6f", "hello".to_string());
        check(a, "80", "".to_string());
    }
}
