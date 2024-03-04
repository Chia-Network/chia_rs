pub mod derivable_key;
pub mod derive_keys;
pub mod error;
pub mod gtelement;
pub mod mnemonic;
pub mod public_key;
pub mod secret_key;
pub mod signature;

pub use derivable_key::DerivableKey;
pub use error::{Error, Result};
pub use gtelement::GTElement;
pub use public_key::{hash_to_g1, hash_to_g1_with_dst, PublicKey};
pub use secret_key::SecretKey;
pub use signature::{
    aggregate, aggregate_pairing, aggregate_verify, hash_to_g2, hash_to_g2_with_dst, sign,
    sign_raw, verify, Signature,
};

pub type G1Element = PublicKey;
pub type G2Element = Signature;
