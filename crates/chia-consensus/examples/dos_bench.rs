//! Adversarial generator DoS benchmark — PR #1371 vs PR #1377.
//!
//! Constructs worst-case generators for both the split cost model (#1371) and
//! the pure storage model (#1377), runs each through run_block_generator2 with
//! the INTERNED_GENERATOR flag, and reports wall-clock timings.
//!
//! Run with:
//!   cargo run --release --example dos_bench -p chia-consensus 2>&1

use chia_bls::Signature;
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::flags::ConsensusFlags;
use chia_consensus::run_block_generator::run_block_generator2;
use clvmr::Allocator;
use clvmr::serde::node_to_bytes;
use std::time::{Duration, Instant};

const MAX_BLOCK_COST: u64 = 11_000_000_000;

/// Build a right-spine cons chain: (nil . (nil . ... nil)) with `n` pairs.
/// The full generator program is (q . (nil . chain)),
/// which evaluates to (nil . chain); first() = nil = empty spend list.
///
/// Cost under pure-storage model:
///   atoms after intern = 2 (nil=0 bytes, q=1 byte)
///   pairs after intern = n + 2
///   cost = (1 + 2*2 + 3*(n+2)) * 12000 = (11 + 3n) * 12000
///
/// Cost under split model:
///   size_cost = 11 + 3n
///   sha_atom_blocks = ceil_sha(0) + ceil_sha(1) = 1 + 1 = 2
///   sha_blocks = 2 + 2*(n+2) = 2n+6
///   sha_invocations = 2 + (n+2) = n+4
///   sha_cost = (2n+6)*1 + (n+4)*8 = 10n+38
///   total = (11+3n)*6000 + (10n+38)*4500 = 237000 + 63000n
fn build_generator(n_chain_pairs: u64) -> Vec<u8> {
    let mut a = Allocator::new();
    let mut tail = a.nil();
    for _ in 0..n_chain_pairs {
        let nil = a.nil();
        tail = a.new_pair(nil, tail).expect("allocator OOM");
    }
    let nil = a.nil();
    let body = a.new_pair(nil, tail).expect("allocator OOM");
    let q_atom = a.new_atom(&[1]).expect("allocator OOM");
    let program = a.new_pair(q_atom, body).expect("allocator OOM");
    node_to_bytes(&a, program).expect("serialization failed")
}

fn median(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[v.len() / 2]
}

fn run_benchmark(label: &str, generator: &[u8], flags: ConsensusFlags, runs: usize) {
    let mut times = Vec::with_capacity(runs);
    let mut last_err: Option<String> = None;

    for _ in 0..runs {
        let t = Instant::now();
        let result = run_block_generator2(
            generator,
            std::iter::empty::<Vec<u8>>(),
            MAX_BLOCK_COST,
            flags,
            &Signature::default(),
            None,
            &TEST_CONSTANTS,
        );
        let elapsed = t.elapsed();
        match result {
            Ok(_) => {
                times.push(elapsed);
            }
            Err(e) => {
                last_err = Some(format!("{e:?}"));
                // still record time even on error
                times.push(elapsed);
            }
        }
    }

    if let Some(ref e) = last_err {
        println!("  WARNING: last run errored: {e}");
    }

    if times.is_empty() {
        println!("  {label}: no successful runs");
        return;
    }

    let med = median(times.clone());
    let worst = *times.iter().max().unwrap();
    let best = *times.iter().min().unwrap();

    println!(
        "  {label}: median={:.1}ms  worst={:.1}ms  best={:.1}ms  (n={})",
        med.as_secs_f64() * 1000.0,
        worst.as_secs_f64() * 1000.0,
        best.as_secs_f64() * 1000.0,
        times.len(),
    );
}

fn main() {
    let runs = 12;

    // --- Adversarial generator calibrated for pure-storage model (#1377) ---
    // cost = (11 + 3n) * 12000 <= MAX_BLOCK_COST
    // n = (MAX / 12000 - 11) / 3
    let n_pure = (MAX_BLOCK_COST / 12_000 - 11) / 3;
    let cost_pure = (11 + 3 * n_pure) * 12_000;
    println!(
        "Building pure-storage adversarial generator: n_chain={n_pure}, estimated_cost={cost_pure}"
    );
    let gen_pure = build_generator(n_pure);
    println!("  serialized size: {} bytes", gen_pure.len());

    // --- Adversarial generator calibrated for split model (#1371) ---
    // total = 237000 + 63000n <= MAX_BLOCK_COST
    // n = (MAX - 237000) / 63000
    let n_split = (MAX_BLOCK_COST - 237_000) / 63_000;
    let cost_split = 237_000 + 63_000 * n_split;
    println!(
        "Building split-model adversarial generator: n_chain={n_split}, estimated_cost={cost_split}"
    );
    let gen_split = build_generator(n_split);
    println!("  serialized size: {} bytes", gen_split.len());

    let flags_interned = ConsensusFlags::INTERNED_GENERATOR
        | ConsensusFlags::SIMPLE_GENERATOR
        | ConsensusFlags::DONT_VALIDATE_SIGNATURE;

    println!("\n=== Benchmarking with INTERNED_GENERATOR flag (both models use this path) ===");
    println!("  runs={runs}  max_cost={MAX_BLOCK_COST}\n");

    println!("[pure-storage adversarial generator — optimized for PR #1377]");
    run_benchmark("INTERNED_GENERATOR", &gen_pure, flags_interned, runs);

    println!("\n[split-model adversarial generator — optimized for PR #1371]");
    run_benchmark("INTERNED_GENERATOR", &gen_split, flags_interned, runs);

    // Also run without interned flag for comparison (legacy path — what #1371 uses
    // without the hard fork flag enabled)
    let flags_legacy = ConsensusFlags::SIMPLE_GENERATOR | ConsensusFlags::DONT_VALIDATE_SIGNATURE;

    println!("\n=== Legacy path (no INTERNED_GENERATOR — pre-fork baseline) ===");
    println!("[pure-storage adversarial generator on legacy path]");
    run_benchmark("legacy", &gen_pure, flags_legacy, runs);

    println!("\n[split-model adversarial generator on legacy path]");
    run_benchmark("legacy", &gen_split, flags_legacy, runs);
}
