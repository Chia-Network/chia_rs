use chia_bls::{
    aggregate_verify, aggregate_verify_gt, hash_to_g2, sign, GTElement, PublicKey, SecretKey,
    Signature,
};
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

    let mut agg_sig = Signature::default();
    let mut gts = Vec::<GTElement>::new();
    let mut pks = Vec::<PublicKey>::new();
    for idx in 0..1000 {
        let derived = sk.derive_hardened(idx as u32);
        let pk = derived.public_key();
        let sig = sign(&derived, msg_small);
        agg_sig.aggregate(&sig);

        let mut augmented = pk.to_bytes().to_vec();
        augmented.extend_from_slice(msg_small);
        gts.push(hash_to_g2(augmented.as_slice()).pair(&pk));
        pks.push(pk);
    }

    c.bench_function("aggregate_verify_gt, small msg", |b| {
        b.iter(|| {
            assert!(aggregate_verify_gt(&agg_sig, &gts));
        });
    });

    c.bench_function("aggregate_verify, small msg", |b| {
        b.iter(|| {
            assert!(aggregate_verify(
                &agg_sig,
                pks.iter().map(|pk| (pk, &msg_small[..]))
            ));
        });
    });

    c.bench_function("verify, small msg", |b| {
        b.iter(|| {
            assert!(chia_bls::verify(&sig_small, &pk, black_box(&msg_small)));
        });
    });
    c.bench_function("verify, 4kiB msg", |b| {
        b.iter(|| {
            assert!(chia_bls::verify(&sig_large, &pk, black_box(&msg_large)));
        });
    });
}

criterion_group!(verify, verify_benchmark);
criterion_main!(verify);
