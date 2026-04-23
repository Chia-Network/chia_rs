//! Deserialize CLVM program bytes that may be classic (incl. backrefs) or serde_2026.

use clvmr::allocator::{Allocator, NodePtr};
use clvmr::error::Result;
use clvmr::serde::{
    SERDE_2026_MAGIC_PREFIX, deserialize_2026, node_from_bytes_backrefs,
};

/// Auto-detect serde_2026 (magic prefix) vs classic/backrefs, matching Python `deser_auto`.
pub fn node_from_bytes_auto(allocator: &mut Allocator, blob: &[u8]) -> Result<NodePtr> {
    if blob.starts_with(&SERDE_2026_MAGIC_PREFIX) {
        deserialize_2026(allocator, blob, usize::MAX, true)
    } else {
        node_from_bytes_backrefs(allocator, blob)
    }
}
