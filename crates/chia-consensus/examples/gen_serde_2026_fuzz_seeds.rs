//! Generate seed inputs for the `serde-2026-size-bound` fuzz target.
//!
//! The target checks that serde_2026 encodings stay within the proven size
//! bound (see `chia_consensus::serde_2026::max_canonical_blob_size`). The
//! bound is only *tight* for specific tree shapes, which coverage-guided
//! fuzzing rarely reaches on its own. This example searches random inputs
//! (through the exact decode path the fuzz target uses) for trees whose
//! encoding comes closest to the bound, and writes the best as seeds, so
//! fuzz runs mutate around the boundary instead of wandering toward it.
//!
//! Deterministic per rand version: fixed RNG seed, so regenerated seeds are
//! reproducible (seed corpora are regenerable scratch, so drift across rand
//! releases is fine).
//!
//! ```sh
//! cargo run --release --example gen_serde_2026_fuzz_seeds -- \
//!     fuzz/corpus/serde-2026-size-bound
//! cargo fuzz run serde-2026-size-bound
//! ```

use chia_consensus::generator_cost::interned_vbytes;
use chia_consensus::serde_2026::SERDE_2026_COMPRESSION_LEVEL;
use clvm_fuzzing::make_tree;
use clvmr::Allocator;
use clvmr::serde::{SERDE_2026_MAGIC_PREFIX, intern_tree, serialize_2026};
use rand::rngs::SmallRng;
use rand::{Rng, RngCore, SeedableRng};
use std::fs;

/// Interpret `data` exactly as the fuzz target does and return
/// (blob_size / bound, blob_size).
fn fullness(data: &[u8]) -> Option<(f64, usize)> {
    let mut u = arbitrary::Unstructured::new(data);
    let _max_cost: u64 = u.arbitrary().ok()?;
    let _cost_per_byte: u64 = u.arbitrary().ok()?;
    let mut a = Allocator::new();
    let (node, _) = make_tree(&mut a, &mut u);
    let blob = serialize_2026(&a, node, SERDE_2026_COMPRESSION_LEVEL).ok()?;
    let tree = intern_tree(&a, node).ok()?;
    let bound = interned_vbytes(&tree) as usize + 5 + SERDE_2026_MAGIC_PREFIX.len();
    Some((blob.len() as f64 / bound as f64, blob.len()))
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "fuzz/corpus/serde-2026-size-bound".to_string());
    fs::create_dir_all(&out).unwrap();

    let mut rng = SmallRng::seed_from_u64(0x9e37_79b9_7f4a_7c15);
    let mut best: Vec<(f64, usize, Vec<u8>)> = Vec::new();

    for round in 0..400_000u64 {
        let len = 16 + (rng.next_u64() % 3000) as usize;
        let mut data = vec![0u8; len];
        rng.fill_bytes(&mut data);
        // Bias some inputs toward long constant runs, which favors large
        // atoms and deep spines over noise.
        if round % 3 == 0 {
            let run_byte = rng.random::<u8>();
            let start = 16 + (rng.next_u64() as usize % (len - 16).max(1)).min(len - 16);
            for b in &mut data[start..] {
                *b = run_byte;
            }
        }
        if let Some((score, size)) = fullness(&data) {
            best.push((score, size, data));
            best.sort_by(|x, y| y.0.total_cmp(&x.0));
            best.truncate(200);
        }
    }

    // Keep the fullest inputs across several blob-size buckets, so seeds
    // aren't all tiny.
    let mut count = 0;
    for (lo, hi) in [(0, 100), (100, 1000), (1000, 10_000), (10_000, usize::MAX)] {
        for (score, size, data) in best.iter().filter(|e| e.1 >= lo && e.1 < hi).take(3) {
            let path = format!("{out}/near-bound-{count:02}");
            fs::write(&path, data).unwrap();
            println!("{path}: fullness {score:.4}, blob {size} bytes");
            count += 1;
        }
    }
}
