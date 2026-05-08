//! Consensus-tuned wrappers around the `clvm_rs::serde_2026` deserializer.
//!
//! `clvm_rs` deliberately makes the caller pick `max_atom_len` and `strict`,
//! since those are policy and clvm_rs has no consensus opinion. This module
//! supplies the values chia consensus expects and exposes the
//! "sniff the magic prefix and dispatch" convenience that callers used to get
//! from `clvm_rs::serde::node_from_bytes_auto`.

use clvmr::allocator::{Allocator, NodePtr};
use clvmr::error::{EvalErr, Result};
use clvmr::serde::{SERDE_2026_MAGIC_PREFIX, deserialize_2026, node_from_bytes_backrefs};

/// Per-atom byte cap used by chia consensus when deserializing CLVM blobs.
///
/// Matches the historical `clvm_rs` default. CLVM atoms above this size are
/// uneconomical to construct under cost limits anyway; this is a defensive
/// pre-allocation cap, not a hard consensus rule.
pub const CONSENSUS_MAX_ATOM_LEN: usize = 1 << 20;

/// Maximum total blob size accepted by consensus deserialization (10 MiB).
pub const CONSENSUS_MAX_BLOB_SIZE: usize = 10 * 1024 * 1024;

/// Deserialize CLVM bytes, auto-detecting classic / backrefs / serde_2026.
///
/// Sniffs `SERDE_2026_MAGIC_PREFIX` at the head of `bytes`; if present,
/// dispatches to [`deserialize_2026`] with consensus caps (atom ≤ 1 MiB,
/// total blob ≤ 10 MiB). Otherwise falls back to [`node_from_bytes_backrefs`]
/// (which also accepts plain classic).
pub fn node_from_bytes_auto(allocator: &mut Allocator, bytes: &[u8]) -> Result<NodePtr> {
    if bytes.len() > CONSENSUS_MAX_BLOB_SIZE {
        return Err(EvalErr::SerializationError);
    }
    if bytes.starts_with(&SERDE_2026_MAGIC_PREFIX) {
        deserialize_2026(allocator, bytes, CONSENSUS_MAX_ATOM_LEN, false)
    } else {
        node_from_bytes_backrefs(allocator, bytes)
    }
}
