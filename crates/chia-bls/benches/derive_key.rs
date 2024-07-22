use chia_bls::{DerivableKey, SecretKey};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::Instant;

fn key_derivation_benchmark(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(1337);
    let mut data = [0u8; 32];
    rng.fill(data.as_mut_slice());

    let sk = SecretKey::from_seed(&data);
    let pk = sk.public_key();

    c.bench_function("secret key, unhardened", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for i in 0..iters {
                black_box(sk.derive_unhardened(i as u32));
            }
            start.elapsed()
        });
    });
    c.bench_function("secret key, hardened", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for i in 0..iters {
                black_box(sk.derive_hardened(i as u32));
            }
            start.elapsed()
        });
    });
    c.bench_function("public key, unhardened", |b| {
        b.iter_custom(|iters| {
            let start = Instant::now();
            for i in 0..iters {
                black_box(pk.derive_unhardened(i as u32));
            }
            start.elapsed()
        });
    });
}

criterion_group!(key_derivation, key_derivation_benchmark);
criterion_main!(key_derivation);
