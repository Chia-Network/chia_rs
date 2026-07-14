#![no_main]
use libfuzzer_sys::{arbitrary, fuzz_target};

use chia_consensus::generator_cost::interned_vbytes;
use chia_consensus::serde_2026::{
    SERDE_2026_COMPRESSION_LEVEL, max_canonical_blob_size, node_from_bytes_auto,
};
use clvm_fuzzing::make_tree;
use clvmr::Allocator;
use clvmr::serde::{SERDE_2026_MAGIC_PREFIX, intern_tree, serialize_2026};

// Empirically checks the theorem behind `max_canonical_blob_size` (see the
// proof on that function; this target hunts for counterexamples):
//
// 1. Encoding bound: for ANY CLVM tree, the canonical serde_2026 wire
//    encoding is at most interned_vbytes(tree) + 5 bytes plus the magic
//    prefix.
// 2. Corollary, at fuzzed cost constants: if the tree is affordable under
//    (max_cost, cost_per_byte), its blob fits in
//    max_canonical_blob_size(max_cost, cost_per_byte).
//
// Also verifies the blob round-trips to the same tree.
//
// The bound is only tight for specific tree shapes; seed the corpus with
// near-bound inputs first (see the gen_serde_2026_fuzz_seeds example in
// chia-consensus) so mutation starts at the boundary.
fuzz_target!(|data: &[u8]| {
    let mut unstructured = arbitrary::Unstructured::new(data);
    let max_cost: u64 = unstructured.arbitrary().unwrap_or(11_000_000_000);
    let cost_per_byte: u64 = unstructured.arbitrary().unwrap_or(12_000);

    let mut a = Allocator::new();
    let (node, _) = make_tree(&mut a, &mut unstructured);

    let blob = serialize_2026(&a, node, SERDE_2026_COMPRESSION_LEVEL).expect("serialize_2026");

    let tree = intern_tree(&a, node).expect("intern_tree");
    let vbytes = interned_vbytes(&tree);
    let bound = vbytes as usize + 5 + SERDE_2026_MAGIC_PREFIX.len();
    assert!(
        blob.len() <= bound,
        "size bound violated: blob {} > interned_vbytes-derived bound {}",
        blob.len(),
        bound
    );

    // Corollary at arbitrary cost constants: any tree affordable under
    // (max_cost, cost_per_byte) must encode within the derived cap.
    if vbytes
        .checked_mul(cost_per_byte)
        .is_some_and(|c| c <= max_cost)
    {
        let cap = max_canonical_blob_size(max_cost, cost_per_byte);
        assert!(
            blob.len() <= cap,
            "cap violated: blob {} > max_canonical_blob_size({max_cost}, {cost_per_byte}) = {cap}",
            blob.len(),
        );
    }

    // Round-trip check via canonical re-serialization: serialize_2026 is
    // deterministic and DAG-aware, so equal trees produce equal blobs.
    // (Comparing classic encodings instead would blow up on trees whose
    // classic expansion is huge — compressing those is the format's point.)
    // Passing `bound` as the size cap doubles as a check that the gate
    // admits every canonical blob.
    let mut b = Allocator::new();
    let parsed = node_from_bytes_auto(&mut b, &blob, bound).expect("node_from_bytes_auto");
    let blob2 = serialize_2026(&b, parsed, SERDE_2026_COMPRESSION_LEVEL).expect("serialize_2026");
    assert_eq!(blob, blob2, "round-trip mismatch");
});
