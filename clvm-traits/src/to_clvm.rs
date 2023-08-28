use clvmr::{allocator::NodePtr, Allocator};

use crate::Result;

pub trait ToClvm {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr>;
}

impl ToClvm for NodePtr {
    fn to_clvm(&self, _a: &mut Allocator) -> Result<NodePtr> {
        Ok(*self)
    }
}

macro_rules! clvm_primitive {
    ($primitive:ty) => {
        impl ToClvm for $primitive {
            fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
                Ok(a.new_number((*self).into())?)
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

impl<T> ToClvm for &T
where
    T: ToClvm,
{
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        T::to_clvm(*self, a)
    }
}

impl<A, B> ToClvm for (A, B)
where
    A: ToClvm,
    B: ToClvm,
{
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        let first = self.0.to_clvm(a)?;
        let rest = self.1.to_clvm(a)?;
        Ok(a.new_pair(first, rest)?)
    }
}

impl ToClvm for () {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        Ok(a.null())
    }
}

impl<T> ToClvm for &[T]
where
    T: ToClvm,
{
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        let mut result = a.null();
        for item in self.iter().rev() {
            let value = item.to_clvm(a)?;
            result = a.new_pair(value, result)?;
        }
        Ok(result)
    }
}

impl<T, const N: usize> ToClvm for [T; N]
where
    T: ToClvm,
{
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        self.as_slice().to_clvm(a)
    }
}

impl<T> ToClvm for Vec<T>
where
    T: ToClvm,
{
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        self.as_slice().to_clvm(a)
    }
}

impl<T> ToClvm for Option<T>
where
    T: ToClvm,
{
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        match self {
            Some(value) => value.to_clvm(a),
            None => Ok(a.null()),
        }
    }
}

impl ToClvm for &str {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        Ok(a.new_atom(self.as_bytes())?)
    }
}

impl ToClvm for String {
    fn to_clvm(&self, a: &mut Allocator) -> Result<NodePtr> {
        self.as_str().to_clvm(a)
    }
}

#[cfg(test)]
mod tests {
    use clvmr::serde::node_to_bytes;
    use hex::ToHex;

    use super::*;

    fn encode<T>(a: &mut Allocator, value: T) -> Result<String>
    where
        T: ToClvm,
    {
        let actual = value.to_clvm(a).unwrap();
        let actual_bytes = node_to_bytes(a, actual).unwrap();
        Ok(actual_bytes.encode_hex())
    }

    fn check<T>(a: &mut Allocator, value: T, hex: &str)
    where
        T: ToClvm,
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
