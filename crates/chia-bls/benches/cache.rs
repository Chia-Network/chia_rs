use chia_bls::aggregate_verify;
use chia_bls::{sign, BlsCache, SecretKey, Signature};
use criterion::{criterion_group, criterion_main, Criterion};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn cache_benchmark(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(1337);
    let mut data = [0u8; 32];
    rng.fill(data.as_mut_slice());

    let sk = SecretKey::from_seed(&data);
    let msg = b"The quick brown fox jumps over the lazy dog";

    let mut pks = Vec::new();

    let mut agg_sig = Signature::default();
    for i in 0..1000 {
        let derived = sk.derive_hardened(i);
        let pk = derived.public_key();
        let sig = sign(&derived, msg);
        agg_sig.aggregate(&sig);
        pks.push(pk);
    }

    let bls_cache = BlsCache::default();
    c.bench_function("bls_cache.aggregate_verify, 0% cache hits", |b| {
        b.iter(|| {
            let bls_cache = bls_cache.clone();
            assert!(bls_cache.aggregate_verify(pks.iter().zip([&msg].iter().cycle()), &agg_sig));
        });
    });

    // populate 10% of keys
    let bls_cache = BlsCache::default();
    bls_cache.aggregate_verify(pks[0..100].iter().zip([&msg].iter().cycle()), &agg_sig);
    c.bench_function("bls_cache.aggregate_verify, 10% cache hits", |b| {
        b.iter(|| {
            let bls_cache = bls_cache.clone();
            assert!(bls_cache.aggregate_verify(pks.iter().zip([&msg].iter().cycle()), &agg_sig));
        });
    });

    // populate another 10% of keys
    let bls_cache = BlsCache::default();
    bls_cache.aggregate_verify(pks[0..200].iter().zip([&msg].iter().cycle()), &agg_sig);
    c.bench_function("bls_cache.aggregate_verify, 20% cache hits", |b| {
        b.iter(|| {
            let bls_cache = bls_cache.clone();
            assert!(bls_cache.aggregate_verify(pks.iter().zip([&msg].iter().cycle()), &agg_sig));
        });
    });

    // populate another 30% of keys
    let bls_cache = BlsCache::default();
    bls_cache.aggregate_verify(pks[0..500].iter().zip([&msg].iter().cycle()), &agg_sig);
    c.bench_function("bls_cache.aggregate_verify, 50% cache hits", |b| {
        b.iter(|| {
            let bls_cache = bls_cache.clone();
            assert!(bls_cache.aggregate_verify(pks.iter().zip([&msg].iter().cycle()), &agg_sig));
        });
    });

    // populate all other keys
    let bls_cache = BlsCache::default();
    bls_cache.aggregate_verify(pks[0..1000].iter().zip([&msg].iter().cycle()), &agg_sig);
    c.bench_function("bls_cache.aggregate_verify, 100% cache hits", |b| {
        b.iter(|| {
            let bls_cache = bls_cache.clone();
            assert!(bls_cache.aggregate_verify(pks.iter().zip([&msg].iter().cycle()), &agg_sig));
        });
    });

    c.bench_function("bls_cache.aggregate_verify, no cache", |b| {
        b.iter(|| {
            assert!(aggregate_verify(
                &agg_sig,
                pks.iter().map(|pk| (pk, &msg[..]))
            ));
        });
    });
}

criterion_group!(cache, cache_benchmark);
criterion_main!(cache);
