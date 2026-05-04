//! Consensus-tuned wrappers around the `clvm_rs::serde_2026` deserializer.
//!
//! `clvm_rs` deliberately makes the caller pick `max_atom_len` and `strict`,
//! since those are policy and clvm_rs has no consensus opinion. This module
//! supplies the values chia consensus expects and exposes the
//! "sniff the magic prefix and dispatch" convenience that callers used to get
//! from `clvm_rs::serde::node_from_bytes_auto`.

use clvmr::allocator::{Allocator, NodePtr};
use clvmr::error::Result;
use clvmr::serde::{SERDE_2026_MAGIC_PREFIX, deserialize_2026, node_from_bytes_backrefs};

/// Per-atom byte cap used by chia consensus when deserializing CLVM blobs.
///
/// Matches the historical `clvm_rs` default. CLVM atoms above this size are
/// uneconomical to construct under cost limits anyway; this is a defensive
/// pre-allocation cap, not a hard consensus rule.
pub const CONSENSUS_MAX_ATOM_LEN: usize = 1 << 20;

/// Deserialize CLVM bytes, auto-detecting classic / backrefs / serde_2026.
///
/// Sniffs `SERDE_2026_MAGIC_PREFIX` at the head of `bytes`; if present,
/// dispatches to [`deserialize_2026`] with consensus caps. Otherwise falls
/// back to [`node_from_bytes_backrefs`] (which also accepts plain classic).
pub fn node_from_bytes_auto(allocator: &mut Allocator, bytes: &[u8]) -> Result<NodePtr> {
    if let Some(body) = bytes.strip_prefix(SERDE_2026_MAGIC_PREFIX.as_slice()) {
        deserialize_2026(allocator, body, CONSENSUS_MAX_ATOM_LEN, false)
    } else {
        node_from_bytes_backrefs(allocator, bytes)
    }
}
