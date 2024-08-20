use std::fmt;

use criterion::{criterion_group, criterion_main, Criterion};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};

#[serde_as]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Coin {
    #[serde_as(as = "Bytes")]
    parent_coin_info: [u8; 32],
    #[serde_as(as = "Bytes")]
    puzzle_hash: [u8; 32],
    amount: u64,
}

fn random_coin(rng: &mut StdRng) -> Coin {
    Coin {
        parent_coin_info: rng.gen(),
        puzzle_hash: rng.gen(),
        amount: rng.gen(),
    }
}

fn to_bytes<T>(coin: &T)
where
    T: Serialize + DeserializeOwned + fmt::Debug + PartialEq,
{
    let bytes = serde_streamable::to_bytes(coin).unwrap();
    let roundtrip = serde_streamable::from_bytes::<T>(&bytes).unwrap();
    assert_eq!(coin, &roundtrip);
}

fn serialize_benchmark(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(1337);

    let coin = random_coin(&mut rng);

    let mut small_list = Vec::new();
    for _ in 0..10_000 {
        small_list.push(random_coin(&mut rng));
    }

    let mut large_list = Vec::new();
    for _ in 0..1_000_000 {
        large_list.push(random_coin(&mut rng));
    }

    c.bench_function("serialize 1 coin", |b| {
        b.iter(|| {
            to_bytes(&coin);
        });
    });

    c.bench_function("serialize 10000 coins", |b| {
        b.iter(|| {
            to_bytes(&small_list);
        });
    });

    c.bench_function("serialize 1000000 coins", |b| {
        b.iter(|| {
            to_bytes(&large_list);
        });
    });
}

criterion_group!(serializing, serialize_benchmark);
criterion_main!(serializing);
