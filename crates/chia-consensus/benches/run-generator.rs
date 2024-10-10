use chia_bls::Signature;
use chia_consensus::consensus_constants::TEST_CONSTANTS;
use chia_consensus::gen::additions_and_removals::additions_and_removals;
use chia_consensus::gen::flags::{ALLOW_BACKREFS, DONT_VALIDATE_SIGNATURE};
use chia_consensus::gen::run_block_generator::{run_block_generator, run_block_generator2};
use clvmr::serde::{node_from_bytes, node_to_bytes_backrefs};
use clvmr::Allocator;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::fs;
use std::time::Instant;

fn run(c: &mut Criterion) {
    let mut group = c.benchmark_group("generator");
    group.sample_size(10);

    for name in &[
        "block-4671894",
        "block-834752",
        "block-225758",
        "deep-recursion-plus",
        "duplicate-coin-announce",
        "block-1ee588dc",
        "block-6fe59b24",
        "block-b45268ac",
        "block-c2a8df0d",
        "block-e5002df2",
        "recursion-pairs",
    ] {
        let filename = format!("../../generator-tests/{name}.txt");
        println!("file: {filename}");
        let test_file = fs::read_to_string(filename).expect("test file not found");
        let generator = test_file.split_once('\n').expect("invalid test file").0;
        let generator = hex::decode(generator).expect("invalid hex encoded generator");

        let mut block_refs = Vec::<Vec<u8>>::new();

        let filename = format!("../../generator-tests/{name}.env");
        if let Ok(env_hex) = fs::read_to_string(&filename) {
            println!("block-ref file: {filename}");
            block_refs.push(hex::decode(env_hex).expect("hex decode env-file"));
        }

        let compressed_generator = {
            let mut a = Allocator::new();
            let input = node_from_bytes(&mut a, &generator).expect("failed to parse input file");
            node_to_bytes_backrefs(&a, input).expect("failed to compress generator")
        };

        for (gen, name_suffix) in &[(generator, ""), (compressed_generator, "-compressed")] {
            group.bench_function(format!("run_block_generator {name}{name_suffix}"), |b| {
                b.iter(|| {
                    let mut a = Allocator::new();
                    let start = Instant::now();

                    let conds = run_block_generator(
                        &mut a,
                        gen,
                        &block_refs,
                        11_000_000_000,
                        ALLOW_BACKREFS | DONT_VALIDATE_SIGNATURE,
                        &Signature::default(),
                        None,
                        &TEST_CONSTANTS,
                    );
                    let _ = black_box(conds);
                    start.elapsed()
                });
            });

            group.bench_function(format!("run_block_generator2 {name}{name_suffix}"), |b| {
                b.iter(|| {
                    let mut a = Allocator::new();
                    let start = Instant::now();

                    let conds = run_block_generator2(
                        &mut a,
                        gen,
                        &block_refs,
                        11_000_000_000,
                        ALLOW_BACKREFS,
                        &Signature::default(),
                        None,
                        &TEST_CONSTANTS,
                    );
                    let _ = black_box(conds);
                    start.elapsed()
                });
            });

            group.bench_function(format!("additions_and_removals {name}{name_suffix}"), |b| {
                b.iter(|| {
                    let start = Instant::now();
                    let results = additions_and_removals(gen, &block_refs, 0, &TEST_CONSTANTS);
                    let _ = black_box(results);
                    start.elapsed()
                });
            });
        }
    }
}

criterion_group!(generator, run);
criterion_main!(generator);
