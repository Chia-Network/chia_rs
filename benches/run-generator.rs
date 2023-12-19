use chia::gen::conditions::MempoolVisitor;
use chia::gen::flags::ALLOW_BACKREFS;
use chia::gen::run_block_generator::{run_block_generator, run_block_generator2};
use clvmr::Allocator;
use criterion::{criterion_group, criterion_main, Criterion, SamplingMode};
use std::fs::read_to_string;
use std::time::Instant;

fn run(c: &mut Criterion) {
    let mut group = c.benchmark_group("generator");
    group.sample_size(20);
    group.sampling_mode(SamplingMode::Flat);

    for name in &["block-4671894", "block-834752", "block-225758"] {
        let filename = format!("generator-tests/{name}.txt");
        println!("file: {filename}");
        let test_file = read_to_string(filename).expect("test file not found");
        let generator = test_file.split_once('\n').expect("invalid test file").0;
        let generator = hex::decode(generator).expect("invalid hex encoded generator");

        let mut block_refs = Vec::<Vec<u8>>::new();

        let filename = format!("generator-tests/{name}.env");
        if let Ok(env_hex) = std::fs::read_to_string(&filename) {
            println!("block-ref file: {filename}");
            block_refs.push(hex::decode(env_hex).expect("hex decode env-file"));
        }

        group.bench_function(format!("run_block_generator {name}"), |b| {
            b.iter(|| {
                let mut a = Allocator::new();
                let start = Instant::now();

                let conds = run_block_generator::<_, MempoolVisitor>(
                    &mut a,
                    &generator,
                    &block_refs,
                    11000000000,
                    ALLOW_BACKREFS,
                );
                assert!(conds.is_ok());
                start.elapsed()
            })
        });

        group.bench_function(format!("run_block_generator2 {name}"), |b| {
            b.iter(|| {
                let mut a = Allocator::new();
                let start = Instant::now();

                let conds = run_block_generator2::<_, MempoolVisitor>(
                    &mut a,
                    &generator,
                    &block_refs,
                    11000000000,
                    ALLOW_BACKREFS,
                );
                assert!(conds.is_ok());
                start.elapsed()
            })
        });
    }
}

criterion_group!(generator, run);
criterion_main!(generator);
