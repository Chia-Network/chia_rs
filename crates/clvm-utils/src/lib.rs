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
mod curry_tree_hash;
mod hash_encoder;
mod tree_hash;

pub use curried_program::*;
pub use curry_tree_hash::*;
pub use hash_encoder::*;
pub use tree_hash::*;
