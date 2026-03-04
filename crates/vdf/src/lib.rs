//! Pure Rust VDF (verifiable delay function) implementation.

pub mod discriminant;
pub mod verifier;

/// Pure Rust implementations (discriminant, verifier).
pub mod pure {
    pub use super::discriminant;
}
