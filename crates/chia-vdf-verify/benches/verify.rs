use chia_vdf_verify::integer::from_bytes_be;
use chia_vdf_verify::verifier::check_proof_of_time_n_wesolowski;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use malachite_nz::integer::Integer;

fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

// Test vector 1: seed="test_seed_chia", disc=512, iters=100
const D1_HEX: &str = "d0cb181074454b32a0e0fc5e65a1d7625ea43756eaa8de13a9c750c79f7aa60151f065cd5775516159c28713c1e74ced6520f8f5c55129f32f865b28cf7fe8e7";
const X_HEX: &str  = "08000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
const P1_HEX: &str = "020020417eb39c4e14954a817af644fc13d086c26dddab8afea12415b5e685f7883f5740ba01cb75220081c8aba7854cbd52010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

// Test vector 2: seed="chia-vdf-rust", disc=512, iters=200
const D2_HEX: &str = "c3ef34d02017540ef26d88057bbfc778da12ed572b99f8707834ed344577c210b1f9287f54a536913177bf5880a4a51b6bfa42445f3fbcd082b695e38c2066d7";
const P2_HEX: &str = "030033205ea6d1ab367757073029f1462eb2fcc79749871d0b576f7a392adac84f56f46100e477d59353376f82a3eb56720d010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

fn bench_verify(c: &mut Criterion) {
    let d1: Integer = -from_bytes_be(&hex_decode(D1_HEX));
    let d2: Integer = -from_bytes_be(&hex_decode(D2_HEX));
    let x_s = hex_decode(X_HEX);
    let p1 = hex_decode(P1_HEX);
    let p2 = hex_decode(P2_HEX);

    let mut group = c.benchmark_group("verify");

    group.bench_with_input(
        BenchmarkId::new("iters", 100),
        &(&d1, &x_s, &p1, 100u64),
        |b, (d, x, p, iters)| {
            b.iter(|| {
                let result = check_proof_of_time_n_wesolowski(d, x, p, *iters, 0);
                assert!(result);
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::new("iters", 200),
        &(&d2, &x_s, &p2, 200u64),
        |b, (d, x, p, iters)| {
            b.iter(|| {
                let result = check_proof_of_time_n_wesolowski(d, x, p, *iters, 0);
                assert!(result);
            });
        },
    );

    group.finish();
}

criterion_group!(benches, bench_verify);
criterion_main!(benches);
