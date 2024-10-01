use chia_bls::sign;
use chia_bls::Signature;
use chia_bls::{PublicKey, SecretKey};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn parse_benchmark(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(1337);
    let mut data = [0u8; 32];
    rng.fill(data.as_mut_slice());

    let sk = SecretKey::from_seed(&data);
    let pk = sk.public_key();
    let msg = b"The quick brown fox jumps over the lazy dog";
    let sig = sign(&sk, msg);

    let sig_bytes = sig.to_bytes();
    let pk_bytes = pk.to_bytes();

    c.bench_function("parse Signature", |b| {
        b.iter(|| {
            let _ = black_box(Signature::from_bytes(&sig_bytes));
        });
    });

    c.bench_function("parse PublicKey", |b| {
        b.iter(|| {
            let _ = black_box(PublicKey::from_bytes(&pk_bytes));
        });
    });

    c.bench_function("parse Signature (unchecked)", |b| {
        b.iter(|| {
            let _ = black_box(Signature::from_bytes_unchecked(&sig_bytes));
        });
    });

    c.bench_function("parse PublicKey (unchecked)", |b| {
        b.iter(|| {
            let _ = black_box(PublicKey::from_bytes_unchecked(&pk_bytes));
        });
    });
}

criterion_group!(parse, parse_benchmark);
criterion_main!(parse);
