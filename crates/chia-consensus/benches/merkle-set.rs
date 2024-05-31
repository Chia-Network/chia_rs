use chia_consensus::merkle_tree::{validate_merkle_proof, MerkleSet};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::iter::zip;
use std::time::Instant;

const NUM_LEAFS: usize = 1000;

fn run(c: &mut Criterion) {
    let mut group = c.benchmark_group("merkle-set");

    let mut rng = SmallRng::seed_from_u64(1337);

    let mut leafs = Vec::<[u8; 32]>::with_capacity(NUM_LEAFS);
    for _ in 0..NUM_LEAFS {
        let mut item = [0_u8; 32];
        rng.fill(&mut item);
        leafs.push(item);
    }

    group.bench_function("from_leafs", |b| {
        b.iter(|| {
            let start = Instant::now();
            let _ = black_box(MerkleSet::from_leafs(&mut leafs));
            start.elapsed()
        });
    });

    // build the tree from the first half of the leafs. The second half are
    // examples of leafs *not* included in the tree, to also cover
    // proofs-of-exclusion
    let tree = MerkleSet::from_leafs(&mut leafs[0..NUM_LEAFS / 2]);

    group.bench_function("generate_proof", |b| {
        b.iter(|| {
            let start = Instant::now();
            for item in &leafs {
                let _ = black_box(tree.generate_proof(item));
            }
            start.elapsed()
        });
    });

    let mut proofs = Vec::<Vec<u8>>::with_capacity(leafs.len());
    for item in &leafs {
        proofs.push(
            tree.generate_proof(item)
                .expect("failed to generate proof")
                .1,
        );
    }

    group.bench_function("parse_proof", |b| {
        b.iter(|| {
            let start = Instant::now();
            for p in &proofs {
                let _ = black_box(MerkleSet::from_proof(p));
            }
            start.elapsed()
        });
    });
    let root = &tree.get_root();

    group.bench_function("validate_merkle_proof", |b| {
        b.iter(|| {
            let start = Instant::now();
            for (p, leaf) in zip(&proofs, &leafs) {
                let _ =
                    black_box(validate_merkle_proof(p, leaf, root).expect("expect valid proof"));
            }
            start.elapsed()
        });
    });
}

criterion_group!(merkle_set, run);
criterion_main!(merkle_set);
