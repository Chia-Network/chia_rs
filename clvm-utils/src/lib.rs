//! # CLVM Utils
//! This provides various commonly needed utilities for working with CLVM values.
//!
//! ## Currying Example
//!
//! ```rust
//! use clvm_utils::CurriedProgram;
//! use clvm_traits::{ToClvm, clvm_curried_args};
//! use clvmr::{Allocator, serde::node_to_bytes};
//!
//! let a = &mut Allocator::new();
//!
//! let program = a.one();
//!
//! let ptr = CurriedProgram {
//!     program,
//!     args: clvm_curried_args!(42, 75),
//! }
//! .to_clvm(a)
//! .unwrap();
//!
//! let hex = hex::encode(node_to_bytes(a, ptr).unwrap());
//!
//! // (a (q . 1) (c (q . 42) (c (q . 75) 1)))
//! assert_eq!(hex, "ff02ffff0101ffff04ffff012affff04ffff014bff01808080");

mod curried_program;
mod tree_hash;

pub use curried_program::*;
pub use tree_hash::*;
/*
    let curry = CurriedProgram {
        program: program.to_clvm(a).unwrap(),
        args: args.clone(),
    }
    .to_clvm(a)
    .unwrap();
    let actual = node_to_bytes(a, curry).unwrap();
    assert_eq!(hex::encode(actual), expected);

    let curried = CurriedProgram::<A>::from_clvm(a, curry).unwrap();
    let round_program = T::from_clvm(a, curried.program).unwrap();
*/
