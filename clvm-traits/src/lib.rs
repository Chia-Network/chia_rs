//! # CLVM Traits
//! This is a library for encoding and decoding Rust values using a CLVM allocator.
//! It provides implementations for every fixed-width signed and unsigned integer type,
//! as well as many other values in the standard library that would be common to encode.

#![cfg_attr(feature = "derive", doc = "\n\n")]
#![cfg_attr(feature = "derive", doc = include_str!("../docs/derive_macros.md"))]

#[cfg(feature = "derive")]
pub use clvm_derive::*;

mod clvm_decoder;
mod clvm_encoder;
mod error;
mod from_clvm;
mod macros;
mod match_byte;
mod to_clvm;
mod wrappers;

pub use clvm_decoder::*;
pub use clvm_encoder::*;
pub use error::*;
pub use from_clvm::*;
pub use match_byte::*;
pub use to_clvm::*;
pub use wrappers::*;

#[cfg(test)]
pub mod tests {
    extern crate self as clvm_traits;

    use super::*;

    #[derive(Clone)]
    pub enum TestNode {
        Atom(usize),
        Pair(Box<TestNode>, Box<TestNode>),
    }

    #[derive(Default)]
    pub struct TestAllocator {
        atoms: Vec<Vec<u8>>,
    }

    impl TestAllocator {
        pub fn new() -> Self {
            TestAllocator::default()
        }

        fn new_atom(&mut self, buf: &[u8]) -> TestNode {
            let idx = self.atoms.len();
            self.atoms.push(buf.to_vec());
            TestNode::Atom(idx)
        }

        fn atom(&self, idx: usize) -> &[u8] {
            self.atoms[idx].as_slice()
        }
    }

    pub fn node_eq(a: &TestAllocator, left: &TestNode, right: &TestNode) -> bool {
        match (left, right) {
            (TestNode::Atom(l), TestNode::Atom(r)) => a.atom(*l) == a.atom(*r),
            (TestNode::Pair(l1, r1), TestNode::Pair(l2, r2)) => {
                node_eq(a, l1, l2) && node_eq(a, r1, r2)
            }
            _ => false,
        }
    }

    pub fn node_to_str(a: &TestAllocator, input: &TestNode) -> String {
        match input {
            TestNode::Atom(v) => {
                let atom = a.atom(*v);
                if atom.is_empty() {
                    "NIL".to_owned()
                } else {
                    hex::encode(atom)
                }
            }
            TestNode::Pair(l, r) => format!("( {} {}", node_to_str(a, l), node_to_str(a, r)),
        }
    }

    pub fn str_to_node<'a>(a: &mut TestAllocator, input: &'a str) -> (&'a str, TestNode) {
        let (first, rest) = if let Some((f, r)) = input.split_once(' ') {
            (f, r)
        } else {
            (input, "")
        };

        println!("\"{first}\" | \"{rest}\"");
        if first == "(" {
            let (rest, left) = str_to_node(a, rest);
            let (rest, right) = str_to_node(a, rest);
            (rest, TestNode::Pair(Box::new(left), Box::new(right)))
        } else if first == "NIL" {
            (rest, a.new_atom(&[]))
        } else {
            (
                rest,
                a.new_atom(hex::decode(first).expect("invalid hex").as_slice()),
            )
        }
    }

    impl ClvmDecoder for TestAllocator {
        type Node = TestNode;

        fn decode_atom(&self, node: &Self::Node) -> Result<&[u8], FromClvmError> {
            match &node {
                TestNode::Atom(v) => Ok(self.atom(*v)),
                _ => Err(FromClvmError::ExpectedAtom),
            }
        }

        fn decode_pair(
            &self,
            node: &Self::Node,
        ) -> Result<(Self::Node, Self::Node), FromClvmError> {
            match &node {
                TestNode::Pair(l, r) => Ok((*l.clone(), *r.clone())),
                _ => Err(FromClvmError::ExpectedPair),
            }
        }

        fn clone_node(&self, node: &Self::Node) -> Self::Node {
            node.clone()
        }
    }

    impl ClvmEncoder for TestAllocator {
        type Node = TestNode;

        fn encode_atom(&mut self, bytes: &[u8]) -> Result<Self::Node, ToClvmError> {
            Ok(self.new_atom(bytes))
        }

        fn encode_pair(
            &mut self,
            first: Self::Node,
            rest: Self::Node,
        ) -> Result<Self::Node, ToClvmError> {
            Ok(TestNode::Pair(Box::new(first), Box::new(rest)))
        }
    }
}

#[cfg(test)]
#[cfg(feature = "derive")]
mod derive_tests {
    extern crate self as clvm_traits;

    use super::*;
    use crate::tests::*;
    use std::fmt::Debug;

    fn check<T>(value: T, expected: &str)
    where
        T: Debug + PartialEq + ToClvm<TestNode> + FromClvm<TestNode>,
    {
        let a = &mut TestAllocator::new();

        let ptr = value.to_clvm(a).unwrap();

        let actual = node_to_str(a, &ptr);
        assert_eq!(expected, actual);

        let round_trip = T::from_clvm(a, ptr).unwrap();
        assert_eq!(value, round_trip);
    }

    fn coerce_into<A, B>(value: A) -> B
    where
        A: ToClvm<TestNode>,
        B: FromClvm<TestNode>,
    {
        let a = &mut TestAllocator::new();
        let ptr = value.to_clvm(a).unwrap();
        B::from_clvm(a, ptr).unwrap()
    }

    #[test]
    fn test_tuple() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
        #[clvm(tuple)]
        struct TupleStruct {
            a: u64,
            b: i32,
        }

        check(TupleStruct { a: 52, b: -32 }, "( 34 e0");
    }

    #[test]
    fn test_list() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
        #[clvm(list)]
        struct ListStruct {
            a: u64,
            b: i32,
        }

        check(ListStruct { a: 52, b: -32 }, "( 34 ( e0 NIL");
    }

    #[test]
    fn test_curry() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
        #[clvm(curry)]
        struct CurryStruct {
            a: u64,
            b: i32,
        }

        check(
            CurryStruct { a: 52, b: -32 },
            "( 04 ( ( 01 34 ( ( 04 ( ( 01 e0 ( 01 NIL NIL",
        );
    }

    #[test]
    fn test_unnamed() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
        #[clvm(tuple)]
        struct UnnamedStruct(String, String);

        check(UnnamedStruct("A".to_string(), "B".to_string()), "( 41 42");
    }

    #[test]
    fn test_newtype() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
        #[clvm(tuple)]
        struct NewTypeStruct(String);

        check(NewTypeStruct("XYZ".to_string()), "58595a");
    }

    #[test]
    fn test_enum() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
        #[clvm(tuple)]
        enum Enum {
            A(i32),
            B { x: i32 },
            C,
        }

        check(Enum::A(32), "( NIL 20");
        check(Enum::B { x: -72 }, "( 01 b8");
        check(Enum::C, "( 02 NIL");
    }

    #[test]
    fn test_explicit_enum() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
        #[clvm(tuple)]
        #[repr(u8)]
        enum Enum {
            A(i32) = 42,
            B { x: i32 } = 34,
            C = 11,
        }

        check(Enum::A(32), "( 2a 20");
        check(Enum::B { x: -72 }, "( 22 b8");
        check(Enum::C, "( 0b NIL");
    }

    #[test]
    fn test_untagged_enum() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
        #[clvm(tuple, untagged)]
        enum Enum {
            A(i32),

            #[clvm(list)]
            B {
                x: i32,
                y: i32,
            },

            #[clvm(curry)]
            C {
                curried_value: String,
            },
        }

        check(Enum::A(32), "20");
        check(Enum::B { x: -72, y: 94 }, "( b8 ( 5e NIL");
        check(
            Enum::C {
                curried_value: "Hello".to_string(),
            },
            "( 04 ( ( 01 48656c6c6f ( 01 NIL",
        );
    }

    #[test]
    fn test_untagged_enum_parsing_order() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
        #[clvm(tuple, untagged)]
        enum Enum {
            // This variant is parsed first, so `B` will never be deserialized.
            A(i32),
            // When `B` is serialized, it will round trip as `A` instead.
            B(i32),
            // `C` will be deserialized as a fallback when the bytes don't deserialize to a valid `i32`.
            C(String),
        }

        // This round trips to the same value, since `A` is parsed first.
        assert_eq!(coerce_into::<Enum, Enum>(Enum::A(32)), Enum::A(32));

        // This round trips to `A` instead of `B`, since `A` is parsed first.
        assert_eq!(coerce_into::<Enum, Enum>(Enum::B(32)), Enum::A(32));

        // This round trips to `A` instead of `C`, since the bytes used to represent
        // this string are also a valid `i32` value.
        assert_eq!(
            coerce_into::<Enum, Enum>(Enum::C("Hi".into())),
            Enum::A(18537)
        );

        // This round trips to `C` instead of `A`, since the bytes used to represent
        // this string exceed the size of `i32`.
        assert_eq!(
            coerce_into::<Enum, Enum>(Enum::C("Hello, world!".into())),
            Enum::C("Hello, world!".into())
        );
    }
}
