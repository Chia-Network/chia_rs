use chia_bls::{sign, SecretKey};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn sign_benchmark(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(1337);
    let mut data = [0u8; 32];
    rng.fill(data.as_mut_slice());

    let sk = SecretKey::from_seed(&data);
    let small_msg = b"The quick brown fox jumps over the lazy dog";
    let large_msg = [42_u8; 4096];

    c.bench_function("sign, small msg", |b| {
        b.iter(|| {
            sign(&sk, black_box(&small_msg));
        });
    });
    c.bench_function("sign, 4kiB msg", |b| {
        b.iter(|| {
            sign(&sk, black_box(&large_msg));
        });
    });
}

criterion_group!(signing, sign_benchmark);
criterion_main!(signing);
