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
mod int_encoding;
mod macros;
mod match_byte;
mod to_clvm;
mod wrappers;

pub use clvm_decoder::*;
pub use clvm_encoder::*;
pub use error::*;
pub use from_clvm::*;
pub use int_encoding::*;
pub use match_byte::*;
pub use to_clvm::*;
pub use wrappers::*;

pub use clvmr::Atom;

#[cfg(test)]
#[cfg(feature = "derive")]
mod derive_tests {
    extern crate self as clvm_traits;

    use super::*;

    use std::fmt::Debug;

    use clvmr::{serde::node_to_bytes, Allocator};

    fn check<T>(value: &T, expected: &str)
    where
        T: Debug + PartialEq + ToClvm<Allocator> + FromClvm<Allocator>,
    {
        let a = &mut Allocator::new();

        let ptr = value.to_clvm(a).unwrap();

        let actual = node_to_bytes(a, ptr).unwrap();
        assert_eq!(expected, hex::encode(actual));

        let round_trip = T::from_clvm(a, ptr).unwrap();
        assert_eq!(value, &round_trip);
    }

    fn coerce_into<A, B>(value: A) -> B
    where
        A: ToClvm<Allocator>,
        B: FromClvm<Allocator>,
    {
        let a = &mut Allocator::new();
        let ptr = value.to_clvm(a).unwrap();
        B::from_clvm(a, ptr).unwrap()
    }

    #[test]
    fn test_list_struct() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(list)]
        struct Struct {
            a: u64,
            b: i32,
        }

        // Includes the nil terminator.
        check(&Struct { a: 52, b: -32 }, "ff34ff81e080");
    }

    #[test]
    fn test_list_struct_with_rest() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(list)]
        struct Struct {
            a: u64,
            #[clvm(rest)]
            b: i32,
        }

        // Does not include the nil terminator.
        check(&Struct { a: 52, b: -32 }, "ff3481e0");
    }

    #[test]
    fn test_solution_struct() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(solution)]
        struct Struct {
            a: u64,
            b: i32,
        }

        // Includes the nil terminator.
        check(&Struct { a: 52, b: -32 }, "ff34ff81e080");

        // Allows additional parameters.
        let mut allocator = Allocator::new();
        let ptr = clvm_list!(100, 200, 300, 400)
            .to_clvm(&mut allocator)
            .unwrap();
        let value = Struct::from_clvm(&allocator, ptr).unwrap();
        assert_eq!(value, Struct { a: 100, b: 200 });

        // Doesn't allow differing types for the actual solution parameters.
        let mut allocator = Allocator::new();
        let ptr = clvm_list!([1, 2, 3], 200, 300)
            .to_clvm(&mut allocator)
            .unwrap();
        Struct::from_clvm(&allocator, ptr).unwrap_err();
    }

    #[test]
    fn test_solution_struct_with_rest() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(solution)]
        struct Struct {
            a: u64,
            #[clvm(rest)]
            b: i32,
        }

        // Does not include the nil terminator.
        check(&Struct { a: 52, b: -32 }, "ff3481e0");

        // Does not allow additional parameters, since it consumes the rest.
        let mut allocator = Allocator::new();
        let ptr = clvm_list!(100, 200, 300, 400)
            .to_clvm(&mut allocator)
            .unwrap();
        Struct::from_clvm(&allocator, ptr).unwrap_err();
    }

    #[test]
    fn test_curry_struct() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(curry)]
        struct Struct {
            a: u64,
            b: i32,
        }

        check(
            &Struct { a: 52, b: -32 },
            "ff04ffff0134ffff04ffff0181e0ff018080",
        );
    }

    #[test]
    fn test_curry_struct_with_rest() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(curry)]
        struct Struct {
            a: u64,
            #[clvm(rest)]
            b: i32,
        }

        check(&Struct { a: 52, b: -32 }, "ff04ffff0134ff81e080");
    }

    #[test]
    fn test_tuple_struct() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(list)]
        struct Struct(String, String);

        check(&Struct("A".to_string(), "B".to_string()), "ff41ff4280");
    }

    #[test]
    fn test_newtype_struct() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(list)]
        struct Struct(#[clvm(rest)] String);

        check(&Struct("XYZ".to_string()), "8358595a");
    }

    #[test]
    fn test_optional() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(list)]
        struct Struct {
            a: u64,
            #[clvm(default)]
            b: Option<i32>,
        }

        check(
            &Struct {
                a: 52,
                b: Some(-32),
            },
            "ff34ff81e080",
        );
        check(&Struct { a: 52, b: None }, "ff3480");
    }

    #[test]
    fn test_default() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(list)]
        struct Struct {
            a: u64,
            #[clvm(default = 42)]
            b: i32,
        }

        check(&Struct { a: 52, b: 32 }, "ff34ff2080");
        check(&Struct { a: 52, b: 42 }, "ff3480");
    }

    #[test]
    fn test_default_owned() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(list)]
        struct Struct {
            a: u64,
            #[clvm(default = "Hello".to_string())]
            b: String,
        }

        check(
            &Struct {
                a: 52,
                b: "World".to_string(),
            },
            "ff34ff85576f726c6480",
        );
        check(
            &Struct {
                a: 52,
                b: "Hello".to_string(),
            },
            "ff3480",
        );
    }

    #[test]
    fn test_constants() {
        #[derive(ToClvm, FromClvm)]
        #[apply_constants]
        #[derive(Debug, PartialEq)]
        #[clvm(list)]
        struct RunTailCondition<P, S> {
            #[clvm(constant = 51)]
            opcode: u8,
            #[clvm(constant = ())]
            blank_puzzle_hash: (),
            #[clvm(constant = -113)]
            magic_amount: i8,
            puzzle: P,
            solution: S,
        }

        check(
            &RunTailCondition {
                puzzle: "puzzle".to_string(),
                solution: "solution".to_string(),
            },
            "ff33ff80ff818fff8670757a7a6c65ff88736f6c7574696f6e80",
        );
    }

    #[test]
    fn test_enum() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(list)]
        enum Enum {
            A(i32),
            B { x: i32 },
            C,
        }

        check(&Enum::A(32), "ff80ff2080");
        check(&Enum::B { x: -72 }, "ff01ff81b880");
        check(&Enum::C, "ff0280");
    }

    #[test]
    fn test_explicit_discriminant() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(list)]
        #[repr(u8)]
        enum Enum {
            A(i32) = 42,
            B { x: i32 } = 34,
            C = 11,
        }

        check(&Enum::A(32), "ff2aff2080");
        check(&Enum::B { x: -72 }, "ff22ff81b880");
        check(&Enum::C, "ff0b80");
    }

    #[test]
    fn test_untagged_enum() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(list, untagged)]
        enum Enum {
            A(i32),
            B {
                x: i32,
                y: i32,
            },
            #[clvm(curry)]
            C {
                curried_value: String,
            },
        }

        check(&Enum::A(32), "ff2080");
        check(&Enum::B { x: -72, y: 94 }, "ff81b8ff5e80");
        check(
            &Enum::C {
                curried_value: "Hello".to_string(),
            },
            "ff04ffff018548656c6c6fff0180",
        );
    }

    #[test]
    fn test_untagged_enum_parsing_order() {
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(list, untagged)]
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

    #[test]
    fn test_custom_crate_name() {
        use clvm_traits as clvm_traits2;
        #[derive(Debug, ToClvm, FromClvm, PartialEq)]
        #[clvm(list, crate_name = clvm_traits2)]
        struct Struct {
            a: u64,
            b: i32,
        }

        check(&Struct { a: 52, b: -32 }, "ff34ff81e080");
    }
}
