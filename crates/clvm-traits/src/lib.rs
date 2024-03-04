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
#[cfg(feature = "derive")]
mod derive_tests {
    extern crate self as clvm_traits;

    use super::*;

    use std::fmt::Debug;

    use clvmr::{serde::node_to_bytes, Allocator, NodePtr};

    fn check<T>(value: T, expected: &str)
    where
        T: Debug + PartialEq + ToClvm<NodePtr> + FromClvm<NodePtr>,
    {
        let a = &mut Allocator::new();

        let ptr = value.to_clvm(a).unwrap();

        let actual = node_to_bytes(a, ptr).unwrap();
        assert_eq!(expected, hex::encode(actual));

        let round_trip = T::from_clvm(a, ptr).unwrap();
        assert_eq!(value, round_trip);
    }

    fn coerce_into<A, B>(value: A) -> B
    where
        A: ToClvm<NodePtr>,
        B: FromClvm<NodePtr>,
    {
        let a = &mut Allocator::new();
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

        check(TupleStruct { a: 52, b: -32 }, "ff3481e0");
    }

    #[test]
    fn test_list() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
        #[clvm(list)]
        struct ListStruct {
            a: u64,
            b: i32,
        }

        check(ListStruct { a: 52, b: -32 }, "ff34ff81e080");
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
            "ff04ffff0134ffff04ffff0181e0ff018080",
        );
    }

    #[test]
    fn test_unnamed() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
        #[clvm(tuple)]
        struct UnnamedStruct(String, String);

        check(UnnamedStruct("A".to_string(), "B".to_string()), "ff4142");
    }

    #[test]
    fn test_newtype() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
        #[clvm(tuple)]
        struct NewTypeStruct(String);

        check(NewTypeStruct("XYZ".to_string()), "8358595a");
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

        check(Enum::A(32), "ff8020");
        check(Enum::B { x: -72 }, "ff0181b8");
        check(Enum::C, "ff0280");
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

        check(Enum::A(32), "ff2a20");
        check(Enum::B { x: -72 }, "ff2281b8");
        check(Enum::C, "ff0b80");
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
        check(Enum::B { x: -72, y: 94 }, "ff81b8ff5e80");
        check(
            Enum::C {
                curried_value: "Hello".to_string(),
            },
            "ff04ffff018548656c6c6fff0180",
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
