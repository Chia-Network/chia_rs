use clvmr::serde::{node_from_bytes, node_from_bytes_backrefs, node_to_bytes_backrefs};
use clvmr::Allocator;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::fs::read_to_string;
use std::time::Instant;

fn run(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree-hash");

    for name in &[
        "block-4671894",
        "block-834752",
        "block-225758",
        "block-c2a8df0d",
        "block-1ee588dc",
        "block-6fe59b24",
        "block-b45268ac",
    ] {
        let filename = format!("../../generator-tests/{name}.txt");
        println!("file: {filename}");
        let test_file = read_to_string(filename).expect("test file not found");
        let generator = test_file.split_once('\n').expect("invalid test file").0;
        let generator = hex::decode(generator).expect("invalid hex encoded generator");

        let compressed_generator = {
            let mut a = Allocator::new();
            let input = node_from_bytes(&mut a, &generator).expect("failed to parse input file");
            node_to_bytes_backrefs(&a, input).expect("failed to compress generator")
        };

        for (gen, name_suffix) in &[(&generator, ""), (&compressed_generator, "-compressed")] {
            let mut a = Allocator::new();
            let gen = node_from_bytes_backrefs(&mut a, gen).expect("parse generator");

            group.bench_function(format!("tree-hash {name}{name_suffix}"), |b| {
                b.iter(|| {
                    let start = Instant::now();
                    let _ = black_box(clvm_utils::tree_hash(&a, gen));
                    start.elapsed()
                });
            });
        }

        for (gen, name_suffix) in &[(&generator, ""), (&compressed_generator, "-compressed")] {
            group.bench_function(format!("tree-hash-from-stream {name}{name_suffix}"), |b| {
                b.iter(|| {
                    let start = Instant::now();
                    let _ = black_box(clvm_utils::tree_hash_from_bytes(gen));
                    start.elapsed()
                });
            });
        }
    }
}

criterion_group!(tree_hash, run);
criterion_main!(tree_hash);
