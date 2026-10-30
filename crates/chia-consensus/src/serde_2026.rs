//! Consensus-tuned wrappers around the `clvm_rs::serde_2026` deserializer.
//!
//! `clvm_rs` deliberately makes the caller pick `max_atom_len` and `strict`,
//! since those are policy and clvm_rs has no consensus opinion. This module
//! supplies the values chia consensus expects. There is deliberately no
//! "sniff the magic prefix and dispatch" convenience here: every caller
//! already knows whether it's reading classic/backrefs or serde_2026 bytes
//! (from `block.version`, a fork-height comparison, or a `ConsensusFlags`
//! value it was handed), so dispatch belongs at the call site, not hidden
//! behind a byte-prefix sniff.

use clvmr::allocator::{Allocator, NodePtr};
use clvmr::error::{EvalErr, Result};
use clvmr::serde::{SERDE_2026_MAGIC_PREFIX, deserialize_2026};

/// Deserialize a generator on the consensus path: the blob must be a
/// magic-prefixed serde_2026 encoding, with no fallback to classic/backrefs
/// parsing — with `INTERNED_SPEND_LIST` active, serde_2026 is the only legal
/// generator encoding. `max_blob_size` bounds the wire size accepted (derive
/// it via [`max_canonical_blob_size`]); it doubles as the per-atom cap, since
/// atoms appear as literals in the canonical serialization, so an atom of
/// length `L` forces a canonical blob of at least `L` bytes. `strict = false`
/// has the same meaning (and rationale) as in [`node_from_bytes_2026_trusted`].
pub fn node_from_bytes_2026(
    allocator: &mut Allocator,
    bytes: &[u8],
    max_blob_size: usize,
) -> Result<NodePtr> {
    if bytes.len() > max_blob_size {
        return Err(EvalErr::SerializationError);
    }
    deserialize_2026(allocator, bytes, max_blob_size, false)
}

/// Compression level passed to [`clvmr::serde::serialize_2026`] when chia
/// produces serde_2026 blobs.
///
/// The level only affects the serializer's effort/output size; every level
/// produces blobs that the one deserializer accepts (like zlib levels).
/// clvmr keeps its `Compression` enum private and takes a bare `u32`,
/// saturating values above the highest implemented level. Level 0 is the
/// fast/left-first encoding (currently the only one implemented).
pub const SERDE_2026_COMPRESSION_LEVEL: u32 = 0;

/// Largest `max_cost / cost_per_byte` (interned vbytes) for which every
/// varint a level-0 encoding contains fits in 3 bytes: atom lengths are at
/// most `vbytes - 2 <= 2^20 - 1`, and every count and index is smaller.
const THREE_BYTE_VARINT_VBYTES: u64 = (1 << 20) + 1;

/// Maximum serde_2026 wire size, in bytes, of the level-0 encoding of any
/// generator whose cost fits within `max_cost` at `cost_per_byte`. Every
/// generator that could be valid under those constants has a level-0
/// encoding no larger than this. A blob above this size is either over cost
/// or encoded less compactly than level 0, and its sender could re-encode it
/// smaller.
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
/// Let a tree have `S` total atom payload bytes, `n` unique non-nil atoms,
/// `nu` = 1 if nil occurs (else 0) and `p` unique pairs, so its interned
/// weight (see
/// [`generator_cost::interned_vbytes`](crate::generator_cost::interned_vbytes))
/// is `v = S + 2*(n + nu) + 3*p`. The level-0 encoding is the 6-byte prefix,
/// the atom-group count, the group headers, `S` payload bytes, the
/// instruction count, `p` one-byte conses and exactly `p + 1` pushes.
/// While `v <= 2^20 + 1`, every varint fits in 3 bytes and a group header
/// costs at most 2 bytes per atom (3 for atoms of 8192+ bytes), so
/// `wire <= S + floor(S/8192) + 2*n + 4*p + 15 <= floor(4*v/3) + 15`
/// (`wire` is an integer, so the floor is free).
///
/// The `4/3` is necessary: a pair is charged 3 vbytes, but it is one cons
/// plus (on average) one push, and a push of an atom index >= 8190 or a pair
/// index >= 8191 is a 3-byte varint. A left-deep chain
/// `c[i] = (c[i-1] . c[i-2])` over nil reaches `floor(4*v/3) + 15 - 8264` for
/// every length in `8193..=349_525` (the chains inside the 3-byte regime); no
/// tree beats `floor(4*v/3) + 15 - min(p + 1, 8191)`.
///
/// Above `2^20 + 1` vbytes (non-mainnet constants only), every varint is at
/// most `W` = varint width of `v` bytes, and `wire <= W*v + 3*W + 6`.
///
/// Consensus charges `v * cost_per_byte` and rejects anything over
/// `max_cost`, so any potentially valid generator has
/// `v <= max_cost / cost_per_byte`, and the bound is monotone in `v`.
///
/// If `cost_per_byte` is 0, bytes are free and no size is over cost, so the
/// bound degenerates to `usize::MAX` (unbounded).
///
/// At mainnet constants (`max_cost` = 11e9, `cost_per_byte` = 12_000) this
/// is 1_222_236 bytes.
pub fn max_canonical_blob_size(max_cost: u64, cost_per_byte: u64) -> usize {
    if cost_per_byte == 0 {
        // Free bytes: no blob size exhausts the budget, so the least upper
        // bound is "unbounded".
        return usize::MAX;
    }
    let prefix = SERDE_2026_MAGIC_PREFIX.len() as u64;
    let vbytes = max_cost / cost_per_byte;
    if vbytes <= THREE_BYTE_VARINT_VBYTES {
        // 9 = atom-group count + instruction count + the push beyond one
        // per pair, each at most 3 bytes
        return (vbytes * 4 / 3 + 9 + prefix) as usize;
    }
    // Varint width of `vbytes`: w bytes hold 7*w - 1 magnitude bits.
    let Some(width) = (1..=8u64).find(|w| vbytes < 1 << (7 * w - 1)) else {
        return usize::MAX;
    };
    // Saturating: a clamped result is still a correct upper bound, and this
    // keeps the function total for extreme (non-mainnet) constants.
    (vbytes
        .saturating_mul(width)
        .saturating_add(3 * width + prefix))
    .try_into()
    .unwrap_or(usize::MAX)
}

/// Deserialize a serde_2026 blob on a *trusted* read path: no magic-prefix
/// sniffing (the caller already knows this blob is serde_2026-encoded, from
/// its own format decision — `block.version`, a fork-height comparison, or a
/// `ConsensusFlags` value — never from the bytes themselves), and no
/// consensus-derived size cap (the caller has already validated this blob,
/// e.g. via [`node_from_bytes_2026`] at block-acceptance time, or produced it
/// itself). The per-atom bound is the blob's own length, same rationale as
/// [`node_from_bytes_2026`].
///
/// Parsing itself is safe even if `bytes` turned out to be untrusted: time
/// and memory are linear in the blob length (every node costs at least one
/// input byte; shared subtrees are shared, not copied). The caveat is
/// downstream: serde_2026 can encode trees whose *expansion* is exponential
/// in the blob size, so anything traversing the result must be DAG-aware
/// (e.g. [`clvm_utils::tree_hash_cached`] rather than the naive `tree_hash`).
///
/// `strict = false` is deliberate, for the same reason as
/// [`node_from_bytes_2026`]: post-HF2 the generator's identity and cost come
/// from the interned tree, not its byte encoding, so overlong (non-minimal)
/// varints don't affect consensus — they only bloat the blob of whoever
/// produced it.
pub fn node_from_bytes_2026_trusted(allocator: &mut Allocator, bytes: &[u8]) -> Result<NodePtr> {
    deserialize_2026(allocator, bytes, bytes.len(), false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator_cost::interned_vbytes;
    use clvm_fuzzing::node_eq_two;
    use clvmr::serde::{
        intern_tree, node_from_bytes_backrefs, node_to_bytes, node_to_bytes_backrefs,
        serialize_2026,
    };
    use rstest::rstest;

    /// Build a small tree with a repeated subtree (so the backrefs and
    /// serde_2026 encodings are both exercised meaningfully).
    fn sample_tree(a: &mut Allocator) -> NodePtr {
        let atom = a.new_atom(b"hello world, this is a test atom").unwrap();
        let pair = a.new_pair(atom, atom).unwrap();
        a.new_pair(pair, pair).unwrap()
    }

    /// The provable per-tree wire-size bound, `floor(4*v/3) + 15` (valid while
    /// `v <= 2^20 + 1`; see the derivation on [`max_canonical_blob_size`]).
    fn encoding_bound(a: &Allocator, node: NodePtr) -> usize {
        let tree = intern_tree(a, node).unwrap();
        let v = interned_vbytes(&tree);
        assert!(v <= THREE_BYTE_VARINT_VBYTES, "outside the 3-byte regime");
        (v * 4 / 3 + 15) as usize
    }

    /// Left-deep chain over nil: `c[0] = (nil . nil)`, `c[1] = (c[0] . nil)`,
    /// `c[i] = (c[i-1] . c[i-2])`. Each pair is one cons plus one back-ref
    /// push to the pair built two steps earlier, so once more than 8191
    /// pairs exist every push is a 3-byte varint: the worst case for the
    /// `4/3` factor.
    fn chain(a: &mut Allocator, len: usize) -> NodePtr {
        let nil = a.nil();
        let mut prev2 = a.new_pair(nil, nil).unwrap();
        let mut prev = a.new_pair(prev2, nil).unwrap();
        for _ in 2..len {
            let next = a.new_pair(prev, prev2).unwrap();
            prev2 = prev;
            prev = next;
        }
        prev
    }

    /// List of `len` distinct pairs whose children are both back-references
    /// into `num_base` shared `(atom_i . nil)` pairs.
    fn shared_pairs_list(a: &mut Allocator, num_base: usize, len: usize) -> NodePtr {
        let nil = a.nil();
        let base: Vec<NodePtr> = (0..num_base)
            .map(|i| {
                let atom = a.new_atom(&(i as u16).to_be_bytes()).unwrap();
                a.new_pair(atom, nil).unwrap()
            })
            .collect();
        let mut list = nil;
        // `len` distinct (x, y) index pairs, deterministic
        for k in 0..len {
            let (x, y) = (k % num_base, (k / num_base + k) % num_base);
            let item = a.new_pair(base[x], base[y]).unwrap();
            list = a.new_pair(item, list).unwrap();
        }
        list
    }

    /// List of `n` distinct 1- or 2-byte atoms, each referenced twice (so
    /// they sort by first use and the last ones get atom indices near `n`).
    fn atom_list(a: &mut Allocator, n: usize) -> NodePtr {
        let mut list = a.nil();
        for i in 0..n {
            let atom = a.new_atom(&(i as u16 + 256).to_be_bytes()).unwrap();
            let item = a.new_pair(atom, atom).unwrap();
            list = a.new_pair(item, list).unwrap();
        }
        list
    }

    /// Adversarial tree shapes: the configurations where the encoding
    /// bound's slack is smallest (each stresses a different header/varint
    /// growth case in the proof on [`max_canonical_blob_size`]).
    fn tight_trees() -> Vec<(Allocator, NodePtr)> {
        let mut trees = Vec::new();

        // Largest atom inside the 3-byte regime (v = 2^20 + 1), and atoms
        // either side of the 2-byte/3-byte length-header boundary.
        for len in [(1 << 20) - 1, 8191, 8192] {
            let mut a = Allocator::new();
            let node = a.new_atom(&vec![0xa5; len]).unwrap();
            trees.push((a, node));
        }
        // ...and several of them, singleton and grouped headers.
        for (len, count) in [(8191, 2), (8192, 2), (8192, 1), (8191, 64)] {
            let mut a = Allocator::new();
            let mut node = a.nil();
            for i in 0..count {
                let mut bytes = vec![0x5a; len];
                bytes[0] = i as u8;
                let atom = a.new_atom(&bytes).unwrap();
                node = a.new_pair(atom, node).unwrap();
            }
            trees.push((a, node));
        }

        // Worst-case chains, either side of the 2-byte/3-byte back-ref
        // boundary (pair index 8190 vs 8191) and well past it.
        for len in [8191, 8192, 8193, 8194, 20_000, 100_000] {
            let mut a = Allocator::new();
            let node = chain(&mut a, len);
            trees.push((a, node));
        }

        // Atom-index boundary: atom index 8189 is the last 2-byte push.
        for n in [8189, 8190, 8191, 8192, 20_000] {
            let mut a = Allocator::new();
            let node = atom_list(&mut a, n);
            trees.push((a, node));
        }

        // Many distinct pairs over shared children.
        let mut a = Allocator::new();
        let node = shared_pairs_list(&mut a, 9000, 60_000);
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
    fn test_encoding_bound_tight_on_chains() {
        // The chain reaches `floor(4*v/3) + 15 - 8264` at every length in
        // 8193..=349_525 (past the back-ref boundary, inside the 3-byte
        // regime; at 524_288 the instruction count needs 4 bytes and the gap
        // is 8263), so the 4/3 factor can't be lowered and the
        // constant is within 8264 bytes of optimal. If this gap changes,
        // the level-0 encoder changed: revisit the derivation on
        // max_canonical_blob_size.
        for len in [8193, 8194, 20_000, 100_000, 300_000, 349_525] {
            let mut a = Allocator::new();
            let node = chain(&mut a, len);
            let blob = serialize_2026(&a, node, SERDE_2026_COMPRESSION_LEVEL).unwrap();
            assert_eq!(encoding_bound(&a, node) - blob.len(), 8264, "len {len}");
        }
    }

    #[test]
    fn test_encoding_bound_random_dags() {
        // Property test: random DAGs mixing atom sizes and sharing, up to
        // ~40k pairs so indices cross the 2-byte/3-byte boundaries.
        use rand::rngs::SmallRng;
        use rand::{Rng, SeedableRng};
        let mut rng = SmallRng::seed_from_u64(0x5eed_2026);
        for _ in 0..60 {
            let mut a = Allocator::new();
            let mut nodes = vec![a.nil()];
            for i in 0..rng.random_range(1..12_000u32) {
                let len = [1, 2, 4, 32, 64, 8192][rng.random_range(0..6)];
                let mut bytes = vec![0u8; len];
                bytes[..4.min(len)].copy_from_slice(&i.to_be_bytes()[..4.min(len)]);
                nodes.push(a.new_atom(&bytes).unwrap());
            }
            for _ in 0..rng.random_range(1..40_000) {
                // bias toward recent nodes (deep sharing) half the time
                let pick = |rng: &mut SmallRng, n: usize| {
                    if rng.random_bool(0.5) {
                        n - 1 - rng.random_range(0..n.min(8200))
                    } else {
                        rng.random_range(0..n)
                    }
                };
                let l = nodes[pick(&mut rng, nodes.len())];
                let r = nodes[pick(&mut rng, nodes.len())];
                nodes.push(a.new_pair(l, r).unwrap());
            }
            let root = *nodes.last().unwrap();
            if interned_vbytes(&intern_tree(&a, root).unwrap()) > THREE_BYTE_VARINT_VBYTES {
                continue;
            }
            let blob = serialize_2026(&a, root, SERDE_2026_COMPRESSION_LEVEL).unwrap();
            assert!(blob.len() <= encoding_bound(&a, root));
        }
    }

    #[test]
    fn test_max_canonical_blob_size_regimes() {
        // Exact values either side of the 3-byte regime boundary, and the
        // cap is monotone in max_cost across it.
        let v = THREE_BYTE_VARINT_VBYTES;
        assert_eq!(max_canonical_blob_size(v, 1), (v * 4 / 3 + 15) as usize);
        assert_eq!(
            max_canonical_blob_size(v + 1, 1),
            ((v + 1) * 4 + 18) as usize
        );
        assert_eq!(max_canonical_blob_size(0, 1), 15);
        assert_eq!(max_canonical_blob_size(1 << 55, 1), usize::MAX);
        let mut points: Vec<u64> = (0..v + 1000).step_by(997).chain(v - 3..v + 3).collect();
        points.sort_unstable();
        let mut last = 0;
        for vb in points {
            let cap = max_canonical_blob_size(vb, 1);
            assert!(cap >= last);
            last = cap;
        }
        // A 2^20-byte atom (v = 2^20 + 2) is just outside the 3-byte regime
        // (its length varint is 4 bytes) and fits the general bound.
        let mut a = Allocator::new();
        let node = a.new_atom(&vec![0xa5; 1 << 20]).unwrap();
        let blob = serialize_2026(&a, node, SERDE_2026_COMPRESSION_LEVEL).unwrap();
        assert_eq!(blob.len(), (1 << 20) + 13);
        assert!(blob.len() <= max_canonical_blob_size((1 << 20) + 2, 1));
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
    fn test_backrefs_reads_classic_and_backrefs() {
        let mut a = Allocator::new();
        let node = sample_tree(&mut a);
        let expected = node_to_bytes(&a, node).unwrap();

        let classic = expected.clone();
        let backrefs = node_to_bytes_backrefs(&a, node).unwrap();

        for blob in [classic, backrefs] {
            let mut b = Allocator::new();
            let parsed = node_from_bytes_backrefs(&mut b, &blob).expect("node_from_bytes_backrefs");
            assert_eq!(node_to_bytes(&b, parsed).unwrap(), expected);
        }
    }

    #[test]
    fn test_node_from_bytes_2026_trusted_reads_serde_2026() {
        let mut a = Allocator::new();
        let node = sample_tree(&mut a);
        let expected = node_to_bytes(&a, node).unwrap();

        let serde2026 = serialize_2026(&a, node, 0).unwrap();
        assert!(serde2026.starts_with(&SERDE_2026_MAGIC_PREFIX));

        let mut b = Allocator::new();
        let parsed =
            node_from_bytes_2026_trusted(&mut b, &serde2026).expect("node_from_bytes_2026_trusted");
        assert_eq!(node_to_bytes(&b, parsed).unwrap(), expected);
    }

    /// The derived cap at the real consensus constants.
    fn mainnet_cap() -> usize {
        use crate::consensus_constants::TEST_CONSTANTS;
        max_canonical_blob_size(
            TEST_CONSTANTS.max_block_cost_clvm,
            TEST_CONSTANTS.cost_per_byte,
        )
    }

    #[test]
    fn test_max_canonical_blob_size_at_real_constants() {
        // Ties the doc-comment number to the real consensus constants so
        // drift gets caught here instead of silently invalidating the bound.
        assert_eq!(mainnet_cap(), 1_222_236);
    }

    #[test]
    fn test_blob_size_cap() {
        // The cost-derived size cap is a consensus concern, enforced only by
        // node_from_bytes_2026. The trusted reader, node_from_bytes_2026_trusted,
        // has no cap.
        let mut a = Allocator::new();
        let node = sample_tree(&mut a);
        let blob = serialize_2026(&a, node, SERDE_2026_COMPRESSION_LEVEL).unwrap();

        // Consensus entry point, one byte over the cap: rejected before any
        // parsing.
        let mut b = Allocator::new();
        assert!(matches!(
            node_from_bytes_2026(&mut b, &blob, blob.len() - 1),
            Err(EvalErr::SerializationError)
        ));

        // At exactly the cap: parses.
        let mut b = Allocator::new();
        let parsed = node_from_bytes_2026(&mut b, &blob, blob.len()).unwrap();
        // Classic serialization of a DAG with shared subtrees expands it, so
        // compare trees directly rather than via their classic byte encoding.
        assert!(node_eq_two(&b, parsed, &a, node));

        // The trusted reader parses the same serde_2026 blob with no cap to
        // trip over...
        let mut b = Allocator::new();
        let parsed = node_from_bytes_2026_trusted(&mut b, &blob).unwrap();
        assert!(node_eq_two(&b, parsed, &a, node));

        // ...and node_from_bytes_backrefs likewise loads classic blobs
        // uncapped: historical blocks must load regardless of current
        // constants.
        let classic = node_to_bytes(&a, node).unwrap();
        let mut b = Allocator::new();
        let parsed = node_from_bytes_backrefs(&mut b, &classic).unwrap();
        assert!(node_eq_two(&b, parsed, &a, node));
    }
}
