use chia_consensus::merkle_tree::MerkleSet;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::time::Instant;

fn run(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle-set");

    let mut rng = SmallRng::seed_from_u64(1337);

    let mut leafs = Vec::<[u8; 32]>::with_capacity(1000);
    for _ in 0..1000 {
        let mut item = [0_u8; 32];
        rng.fill(&mut item);
        leafs.push(item);
    }

    group.bench_function("from_leafs", |b| {
        b.iter(|| {
            let start = Instant::now();
            let _ = black_box(MerkleSet::from_leafs(&mut leafs));
            start.elapsed()
        })
    });

    let tree = MerkleSet::from_leafs(&mut leafs);

    group.bench_function("generate_proof", |b| {
        b.iter(|| {
            let start = Instant::now();
            for item in &leafs {
                let _ = black_box(tree.generate_proof(&item));
            }
            start.elapsed()
        })
    });

    let mut proofs = Vec::<Vec<u8>>::with_capacity(leafs.len());
    for item in &leafs {
        proofs.push(
            tree.generate_proof(&item)
                .expect("failed to generate proof")
                .expect("item not found"),
        );
    }

    group.bench_function("deserialize_proof", |b| {
        b.iter(|| {
            let start = Instant::now();
            for p in &proofs {
                let _ = black_box(MerkleSet::from_proof(&p));
            }
            start.elapsed()
        })
    });
}

criterion_group!(merkle_set, run);
criterion_main!(merkle_set);
