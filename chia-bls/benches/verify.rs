use chia_bls::secret_key::SecretKey;
use chia_bls::signature;
use chia_bls::signature::sign;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn verify_benchmark(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(1337);
    let mut data = [0u8; 32];
    rng.fill(data.as_mut_slice());

    let sk = SecretKey::from_seed(&data);
    let pk = sk.public_key();
    let msg_small = b"The quick brown fox jumps over the lazy dog";
    let msg_large = [42_u8; 4096];
    let sig_small = sign(&sk, msg_small);
    let sig_large = sign(&sk, msg_large);

    c.bench_function("verify, small msg", |b| {
        b.iter(|| {
            signature::verify(&sig_small, &pk, black_box(&msg_small));
        });
    });
    c.bench_function("verify, 4kiB msg", |b| {
        b.iter(|| {
            signature::verify(&sig_large, &pk, black_box(&msg_large));
        });
    });
}

criterion_group!(verify, verify_benchmark);
criterion_main!(verify);
