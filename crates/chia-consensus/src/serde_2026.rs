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

/// Compression level passed to [`clvmr::serde::serialize_2026`] when chia
/// produces serde_2026 blobs.
///
/// The level only affects the serializer's effort/output size; every level
/// produces blobs that the one deserializer accepts (like zlib levels).
/// clvmr keeps its `Compression` enum private and takes a bare `u32`,
/// saturating values above the highest implemented level. Level 0 is the
/// fast/left-first encoding (currently the only one implemented).
pub const SERDE_2026_COMPRESSION_LEVEL: u32 = 0;

/// Maximum serde_2026 wire size, in bytes, of any generator whose cost fits
/// within `max_cost` at `cost_per_byte` — i.e. every generator that could
/// possibly be valid under those constants has a canonical encoding no
/// larger than this. A blob above this size is either over-cost or
/// non-minimally encoded (and its sender could re-encode it smaller).
///
/// # Why such a bound matters
///
/// Under the interned cost model, cost is charged on the *deduplicated*
/// tree, but the decoder must process every *wire* byte before any cost is
/// charged. Cost alone therefore does not bound pre-charge decoding work; a
/// size cap derived from this function does.
///
/// # Derivation
///
/// Let a tree have `atom_bytes` total atom payload, `U_a` unique atoms and
/// `U_p` unique pairs, so its interned weight (see
/// [`generator_cost::interned_vbytes`](crate::generator_cost::interned_vbytes))
/// is `vbytes = atom_bytes + 2*U_a + 3*U_p`.
///
/// **Step 1 — encoding bound:** the canonical serde_2026 *body* is at most
/// `vbytes + 5` bytes. Sketch: the atom table costs at most 2 bytes of
/// overhead per atom (length varint, amortized group headers) plus a group
/// count header; the instruction stream is exactly `2*U_p + 1` instructions
/// (each push adds one stack entry, each cons nets -1, one root remains),
/// costing 1 byte per cons and at most 2 bytes per push, plus a count
/// header; the headers and per-item slack together never exceed the `+5`
/// because `U_a <= U_p + 1` forces cheap 1-byte pushes to exist whenever the
/// headers grow. The bound is tight (slack reaches 0 at atom length 2^20)
/// and requires atom lengths < 2^27, guaranteed by
/// [`CONSENSUS_MAX_ATOM_LEN`]. The wire blob adds the
/// [`SERDE_2026_MAGIC_PREFIX`] on top of the body.
///
/// **Step 2 — cost bound:** consensus charges `vbytes * cost_per_byte`, and
/// rejects anything over `max_cost`, so any potentially-valid generator has
/// `vbytes <= max_cost / cost_per_byte`.
///
/// Combining: `wire_size <= max_cost / cost_per_byte + 5 + prefix_len`.
///
/// If `cost_per_byte` is 0, bytes are free and no size is over-cost, so the
/// bound degenerates to `usize::MAX` (unbounded).
///
/// At mainnet constants (`max_cost` = 11e9, `cost_per_byte` = 12_000) this
/// is 916_677 bytes, a little under 1 MiB.
pub fn max_canonical_blob_size(max_cost: u64, cost_per_byte: u64) -> usize {
    if cost_per_byte == 0 {
        // Free bytes: no blob size exhausts the budget, so the least upper
        // bound is "unbounded".
        return usize::MAX;
    }
    // Saturating: a clamped result is still a correct upper bound, and this
    // keeps the function total for extreme (non-mainnet) constants.
    ((max_cost / cost_per_byte) as usize).saturating_add(5 + SERDE_2026_MAGIC_PREFIX.len())
}

/// Maximum total blob size accepted by consensus deserialization.
///
/// The principled floor for this value is [`max_canonical_blob_size`] at the
/// network's cost constants (~917 KB on mainnet) — see its doc for the
/// proof. The current value is ~11x that floor: it was inherited from
/// clvm_rs's `DeserializeLimits` default, and the extra headroom admits
/// non-minimal encodings (accepted because `strict = false` below). Whether
/// to tighten it toward the floor, and whether a size cap belongs here or at
/// the peer-protocol layer, are deliberately left open for review.
pub const CONSENSUS_MAX_BLOB_SIZE: usize = 10 * 1024 * 1024;

/// Deserialize CLVM bytes, auto-detecting classic / backrefs / serde_2026.
///
/// Sniffs `SERDE_2026_MAGIC_PREFIX` at the head of `bytes`; if present,
/// dispatches to [`deserialize_2026`] with consensus caps
/// ([`CONSENSUS_MAX_ATOM_LEN`], [`CONSENSUS_MAX_BLOB_SIZE`]). Otherwise
/// falls back to [`node_from_bytes_backrefs`] (which also accepts plain
/// classic).
pub fn node_from_bytes_auto(allocator: &mut Allocator, bytes: &[u8]) -> Result<NodePtr> {
    if bytes.len() > CONSENSUS_MAX_BLOB_SIZE {
        return Err(EvalErr::SerializationError);
    }
    if bytes.starts_with(&SERDE_2026_MAGIC_PREFIX) {
        // strict = false is deliberate. Post-HF2 the generator's identity and
        // cost come from the interned tree, not its byte encoding, so overlong
        // (non-minimal) varints don't affect consensus — they only bloat the
        // blob of whoever produced it. We accept such blobs rather than
        // rejecting valid transactions over a self-inflicted encoding choice;
        // a node is free to re-encode strictly before relaying, and to
        // disconnect a peer that habitually sends non-minimal encodings.
        deserialize_2026(allocator, bytes, CONSENSUS_MAX_ATOM_LEN, false)
    } else {
        node_from_bytes_backrefs(allocator, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator_cost::interned_vbytes;
    use clvmr::serde::{intern_tree, node_to_bytes, node_to_bytes_backrefs, serialize_2026};
    use rstest::rstest;

    /// Build a small tree with a repeated subtree (so the backrefs and
    /// serde_2026 encodings are both exercised meaningfully).
    fn sample_tree(a: &mut Allocator) -> NodePtr {
        let atom = a.new_atom(b"hello world, this is a test atom").unwrap();
        let pair = a.new_pair(atom, atom).unwrap();
        a.new_pair(pair, pair).unwrap()
    }

    /// The provable per-tree wire-size bound: interned_vbytes + 5 + prefix.
    fn encoding_bound(a: &Allocator, node: NodePtr) -> usize {
        let tree = intern_tree(a, node).unwrap();
        interned_vbytes(&tree) as usize + 5 + SERDE_2026_MAGIC_PREFIX.len()
    }

    /// Adversarial tree shapes: the configurations where the encoding
    /// bound's slack is smallest (each stresses a different header/varint
    /// growth case in the proof on [`max_canonical_blob_size`]).
    fn tight_trees() -> Vec<(Allocator, NodePtr)> {
        let mut trees = Vec::new();

        // Single atom at the max length: the bound is exactly tight here.
        let mut a = Allocator::new();
        let node = a.new_atom(&vec![0xa5; CONSENSUS_MAX_ATOM_LEN]).unwrap();
        trees.push((a, node));

        // >63 distinct atom lengths: forces a 2-byte atom-group-count header.
        let mut a = Allocator::new();
        let mut node = a.nil();
        for len in 0..=63usize {
            let atom = a.new_atom(&vec![0x5a; len]).unwrap();
            node = a.new_pair(atom, node).unwrap();
        }
        trees.push((a, node));

        // >4096 unique pairs: forces a 3-byte instruction-count header, and
        // pushes/back-references beyond the 1-byte varint range.
        let mut a = Allocator::new();
        let mut node = a.nil();
        for i in 1..=5000u32 {
            let atom = a.new_number(i.into()).unwrap();
            node = a.new_pair(atom, node).unwrap();
        }
        trees.push((a, node));

        // Doubling DAG: maximal sharing, so wire bytes come almost entirely
        // from back-references rather than atom payload.
        let mut a = Allocator::new();
        let mut node = a.one();
        for _ in 0..20 {
            node = a.new_pair(node, node).unwrap();
        }
        trees.push((a, node));

        // Small mixed tree.
        let mut a = Allocator::new();
        let node = sample_tree(&mut a);
        trees.push((a, node));

        trees
    }

    #[test]
    fn test_encoding_bound_holds_on_tight_shapes() {
        for (a, node) in tight_trees() {
            let blob = serialize_2026(&a, node, SERDE_2026_COMPRESSION_LEVEL).unwrap();
            assert!(
                blob.len() <= encoding_bound(&a, node),
                "encoding bound violated: blob {} > bound {}",
                blob.len(),
                encoding_bound(&a, node)
            );
        }
    }

    #[test]
    fn test_encoding_bound_tight_at_max_atom_len() {
        // A single atom of exactly CONSENSUS_MAX_ATOM_LEN (2^20) bytes is the
        // known worst case: the encoding uses every byte the bound allows.
        // If this stops being exact, the "+5" analysis has changed — revisit
        // the proof on max_canonical_blob_size.
        let mut a = Allocator::new();
        let node = a.new_atom(&vec![0xa5; CONSENSUS_MAX_ATOM_LEN]).unwrap();
        let blob = serialize_2026(&a, node, SERDE_2026_COMPRESSION_LEVEL).unwrap();
        assert_eq!(blob.len(), encoding_bound(&a, node));
    }

    #[rstest]
    // mainnet constants
    #[case(11_000_000_000, 12_000)]
    // tiny budget: only trivial trees are affordable
    #[case(100, 1)]
    // zero budget: nothing is affordable, cap is just the fixed overhead
    #[case(0, 7)]
    // free bytes / huge budget extremes
    #[case(u64::MAX, 1)]
    #[case(u64::MAX, u64::MAX)]
    // zero cost per byte: everything is affordable, cap must be unbounded
    #[case(11_000_000_000, 0)]
    #[case(0, 0)]
    // awkward non-divisible pair
    #[case(1_000_003, 17)]
    fn test_max_canonical_blob_size_general(#[case] max_cost: u64, #[case] cost_per_byte: u64) {
        // The theorem is generic over the cost constants: for ANY
        // (max_cost, cost_per_byte), every tree affordable under them
        // encodes within the derived cap.
        let cap = max_canonical_blob_size(max_cost, cost_per_byte);
        for (a, node) in tight_trees() {
            let tree = intern_tree(&a, node).unwrap();
            let affordable = interned_vbytes(&tree)
                .checked_mul(cost_per_byte)
                .is_some_and(|cost| cost <= max_cost);
            if affordable {
                let blob = serialize_2026(&a, node, SERDE_2026_COMPRESSION_LEVEL).unwrap();
                assert!(
                    blob.len() <= cap,
                    "cap violated at ({max_cost}, {cost_per_byte}): blob {} > cap {cap}",
                    blob.len(),
                );
            }
        }
    }

    #[test]
    fn test_auto_dispatch_all_formats() {
        let mut a = Allocator::new();
        let node = sample_tree(&mut a);
        let expected = node_to_bytes(&a, node).unwrap();

        let classic = expected.clone();
        let backrefs = node_to_bytes_backrefs(&a, node).unwrap();
        let serde2026 = serialize_2026(&a, node, 0).unwrap();
        assert!(serde2026.starts_with(&SERDE_2026_MAGIC_PREFIX));

        for blob in [classic, backrefs, serde2026] {
            let mut b = Allocator::new();
            let parsed = node_from_bytes_auto(&mut b, &blob).expect("node_from_bytes_auto");
            assert_eq!(node_to_bytes(&b, parsed).unwrap(), expected);
        }
    }

    #[test]
    fn test_max_canonical_blob_size_at_real_constants() {
        use crate::consensus_constants::TEST_CONSTANTS;
        // Ties the doc-comment numbers to the real consensus constants so
        // drift gets caught here instead of silently invalidating the bound.
        let bound = max_canonical_blob_size(
            TEST_CONSTANTS.max_block_cost_clvm,
            TEST_CONSTANTS.cost_per_byte,
        );
        assert_eq!(bound, 916_677);
        assert!(CONSENSUS_MAX_BLOB_SIZE >= bound);
    }

    #[test]
    fn test_blob_size_cap() {
        // One byte over the cap must be rejected before any parsing,
        // regardless of format.
        let mut blob = vec![0u8; CONSENSUS_MAX_BLOB_SIZE + 1];
        blob[..SERDE_2026_MAGIC_PREFIX.len()].copy_from_slice(&SERDE_2026_MAGIC_PREFIX);
        let mut a = Allocator::new();
        assert!(matches!(
            node_from_bytes_auto(&mut a, &blob),
            Err(EvalErr::SerializationError)
        ));

        // At exactly the cap, the size gate lets the blob through to the
        // parser (an all-zeros blob happens to parse on the backrefs path:
        // leading 0x00 is a valid atom).
        let blob = vec![0u8; CONSENSUS_MAX_BLOB_SIZE];
        let mut a = Allocator::new();
        assert!(node_from_bytes_auto(&mut a, &blob).is_ok());

        // A valid small blob parses.
        let mut a = Allocator::new();
        let node = sample_tree(&mut a);
        let blob = serialize_2026(&a, node, 0).unwrap();
        let mut b = Allocator::new();
        let parsed = node_from_bytes_auto(&mut b, &blob).unwrap();
        assert_eq!(
            node_to_bytes(&b, parsed).unwrap(),
            node_to_bytes(&a, node).unwrap()
        );
    }
}
