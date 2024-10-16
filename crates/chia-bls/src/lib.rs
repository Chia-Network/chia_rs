#![allow(unsafe_code)]

mod bls_cache;
mod derive_keys;
mod error;
mod gtelement;
mod public_key;
mod secret_key;
mod signature;

#[cfg(feature = "py-bindings")]
mod parse_hex;

pub use bls_cache::BlsCache;
pub use derive_keys::*;
pub use error::{Error, Result};
pub use gtelement::GTElement;
pub use public_key::{hash_to_g1, hash_to_g1_with_dst, PublicKey};
pub use secret_key::SecretKey;
pub use signature::{
    aggregate, aggregate_pairing, aggregate_verify, aggregate_verify_gt, hash_to_g2,
    hash_to_g2_with_dst, sign, sign_raw, verify, Signature,
};

pub type G1Element = PublicKey;
pub type G2Element = Signature;
