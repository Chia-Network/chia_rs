//! # CLVM Traits
//! This is a library for encoding and decoding Rust values using a CLVM allocator.
//! It provides implementations for every fixed-width signed and unsigned integer type,
//! as well as many other values in the standard library that would be common to encode.
//!
//! As well as the built-in implementations, this library exposes two derive macros
//! for implementing the `ToClvm` and `FromClvm` traits on structs. They be marked
//! with one of the following encodings:
//!
//! * `#[clvm(tuple)]` for unterminated lists such as `(A . (B . C))`.
//! * `#[clvm(proper_list)]` for proper lists such as `(A B C)`, or in other words `(A . (B . (C . ())))`.
//! * `#[clvm(curried_args)]` for curried arguments such as `(c (q . A) (c (q . B) (c (q . C) 1)))`.

#![cfg_attr(
    feature = "derive",
    doc = r#"
## Derive Example

```rust
use clvmr::Allocator;
use clvm_traits::{ToClvm, FromClvm};

#[derive(Debug, PartialEq, Eq, ToClvm, FromClvm)]
#[clvm(tuple)]
struct Point {
    x: i32,
    y: i32,
}

let a = &mut Allocator::new();

let point = Point { x: 5, y: 2 };
let ptr = point.to_clvm(a).unwrap();

assert_eq!(Point::from_clvm(a, ptr).unwrap(), point);
```
"#
)]

#[cfg(feature = "derive")]
pub use clvm_derive::*;

mod error;
mod from_clvm;
mod macros;
mod match_byte;
mod to_clvm;

pub use error::*;
pub use from_clvm::*;
pub use macros::*;
pub use match_byte::*;
pub use to_clvm::*;

#[cfg(test)]
#[cfg(feature = "derive")]
mod tests {
    extern crate self as clvm_traits;

    use clvmr::Allocator;

    use super::*;

    #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
    #[clvm(tuple)]
    struct TupleStruct {
        a: u64,
        b: i32,
    }

    #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
    #[clvm(proper_list)]
    struct ProperListStruct {
        a: u64,
        b: i32,
    }

    #[derive(Debug, ToClvm, FromClvm, PartialEq, Eq)]
    #[clvm(curried_args)]
    struct CurriedArgsStruct {
        a: u64,
        b: i32,
    }

    #[test]
    fn test_round_trip_tuple() {
        let mut a = Allocator::new();
        let value = TupleStruct { a: 52, b: -32 };
        let node = value.to_clvm(&mut a).unwrap();
        let round_trip = TupleStruct::from_clvm(&a, node).unwrap();
        assert_eq!(value, round_trip);
    }

    #[test]
    fn test_round_trip_proper_list() {
        let mut a = Allocator::new();
        let value = ProperListStruct { a: 52, b: -32 };
        let node = value.to_clvm(&mut a).unwrap();
        let round_trip = ProperListStruct::from_clvm(&a, node).unwrap();
        assert_eq!(value, round_trip);
    }

    #[test]
    fn test_round_trip_curried_args() {
        let mut a = Allocator::new();
        let value = CurriedArgsStruct { a: 52, b: -32 };
        let node = value.to_clvm(&mut a).unwrap();
        let round_trip = CurriedArgsStruct::from_clvm(&a, node).unwrap();
        assert_eq!(value, round_trip);
    }
}
