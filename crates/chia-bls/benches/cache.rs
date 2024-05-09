use criterion::{criterion_group, criterion_main, Criterion};
use chia_bls::{BLSCache, PublicKey, Signature, SecretKey, aggregate, sign};
use chia_bls::aggregate_verify as agg_ver;

fn cache_benchmark(c: &mut Criterion) {
    let mut bls_cache: BLSCache = BLSCache::default();  // 50000
    // benchmark at 100% cache hit rate
    let mut pk_list: Vec<PublicKey> = [].to_vec();
    let mut msg_list: Vec<&[u8]> = vec![];
    let mut aggsig: Option<Signature> = None;
    for i in 0..=2000 { //cache is half full
        let byte_array: [u8; 32] = [i as u8; 32];
        let sk: SecretKey = SecretKey::from_seed(&byte_array);
        let pk: PublicKey = sk.public_key();
        let msg: &[u8] = &[106; 32];
        let sig: Signature = sign(&sk, msg);
        pk_list.push(pk.clone());
        msg_list.push(msg);
        assert!(bls_cache.aggregate_verify([pk].iter(), [msg].iter(), &sig));
        if aggsig.is_none() {aggsig = Some(sig);} else {aggsig = Some(aggregate([aggsig.unwrap(), sig]));}
    }

    c.bench_function("bls_cache.aggregate_verify, 100% cache hit", |b| {
        b.iter(|| {
            assert!(bls_cache.aggregate_verify(pk_list.iter(), msg_list.iter(), &aggsig.clone().unwrap()));
        });
    });
    let full_aggsig = aggsig.clone().unwrap();

    let mut bls_cache = BLSCache::default();
    let mut aggsig: Option<Signature> = None;
    for i in 0..=1000 {
        let byte_array: [u8; 32] = [i as u8; 32];
        let sk: SecretKey = SecretKey::from_seed(&byte_array);
        let sig: Signature = sign(&sk, msg_list[i as usize]);
        if aggsig.is_none() {aggsig = Some(sig.clone());} else {aggsig = Some(aggregate([aggsig.unwrap(), sig.clone()]));}
        assert!(bls_cache.aggregate_verify([pk_list[i as usize].clone()].iter(), [msg_list[i as usize]].iter(), &sig));
    }

    c.bench_function("bls_cache.aggregate_verify, 50% cache hit", |b| {
        b.iter(|| {
            assert!(bls_cache.clone().aggregate_verify(pk_list.iter(), msg_list.iter(), &full_aggsig.clone()));
        });
    });

    let bls_cache = BLSCache::default();
    let mut aggsig: Option<Signature> = None;
    for i in 0..=500 {
        let byte_array: [u8; 32] = [i as u8; 32];
        let sk: SecretKey = SecretKey::from_seed(&byte_array);
        let sig: Signature = sign(&sk, msg_list[i as usize]);
        if aggsig.is_none() {aggsig = Some(sig.clone());} else {aggsig = Some(aggregate([aggsig.unwrap(), sig.clone()]));}
        assert!(bls_cache.clone().aggregate_verify([pk_list[i as usize].clone()].iter(), [msg_list[i as usize]].iter(), &sig));
    }
    
    c.bench_function("bls_cache.aggregate_verify, 25% cache hit", |b| {
        b.iter(|| {
            assert!(bls_cache.clone().aggregate_verify(pk_list.iter(), msg_list.iter(), &full_aggsig.clone()));
        });
    });

    let bls_cache = BLSCache::default();
    c.bench_function("bls_cache.aggregate_verify, 0% cache hit", |b| {
        b.iter(|| {
            assert!(bls_cache.clone().aggregate_verify(pk_list.iter(), msg_list.iter(), &full_aggsig.clone()));
        });
    });

    let mut data = Vec::<(PublicKey, &[u8])>::new();
    for (pk, msg) in pk_list.iter().zip(msg_list.iter()) {
        data.push((pk.clone(), msg.as_ref()));
    }
    c.bench_function("agg_ver, no cache", |b| {
        b.iter(|| {
            assert!(agg_ver(&full_aggsig, data.clone()));
        });
    });
}

criterion_group!(cache, cache_benchmark);
criterion_main!(cache);