use chia_vdf_verify::discriminant::create_discriminant;
use chia_vdf_verify::integer::from_bytes_be;
use chia_vdf_verify::verifier::check_proof_of_time_n_wesolowski;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use malachite_nz::integer::Integer;
use std::time::Duration;

fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

struct VdfVec {
    challenge: Vec<u8>,
    disc_bits: usize,
    input: Vec<u8>,
    proof: Vec<u8>,
    iters: u64,
    depth: u64,
}

fn parse_vdf_txt(content: &str) -> Vec<VdfVec> {
    let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();
    lines
        .chunks(6)
        .filter(|c| c.len() == 6)
        .map(|c| VdfVec {
            challenge: hex_decode(c[0]),
            disc_bits: c[1].parse().unwrap(),
            input: hex_decode(c[2]),
            proof: hex_decode(c[3]),
            iters: c[4].parse().unwrap(),
            depth: c[5].parse().unwrap(),
        })
        .collect()
}

// 512-bit discriminant test vectors (fast, for regression tracking)
const D1_HEX: &str = "d0cb181074454b32a0e0fc5e65a1d7625ea43756eaa8de13a9c750c79f7aa60151f065cd5775516159c28713c1e74ced6520f8f5c55129f32f865b28cf7fe8e7";
const X_HEX: &str = "08000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
const P1_HEX: &str = "020020417eb39c4e14954a817af644fc13d086c26dddab8afea12415b5e685f7883f5740ba01cb75220081c8aba7854cbd52010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

fn bench_verify(c: &mut Criterion) {
    // --- 512-bit disc, 100 iters (fast regression check) ---
    let d1: Integer = -from_bytes_be(&hex_decode(D1_HEX));
    let x_s = hex_decode(X_HEX);
    let p1 = hex_decode(P1_HEX);

    let mut group = c.benchmark_group("verify");

    group.bench_with_input(
        BenchmarkId::new("512bit_100iters", ""),
        &(&d1, &x_s, &p1, 100u64),
        |b, (d, x, p, iters)| {
            b.iter(|| {
                let result = check_proof_of_time_n_wesolowski(d, x, p, *iters, 0);
                assert!(result);
            });
        },
    );

    // --- 1024-bit disc, real-world iteration counts from vdf.txt ---
    let vdf_txt = include_str!("../tests/fixtures/vdf.txt");
    let vecs = parse_vdf_txt(vdf_txt);

    // depth-0: single Wesolowski proof, ~130M iterations, 1024-bit disc
    if let Some(v) = vecs.iter().find(|v| v.depth == 0) {
        let d = create_discriminant(&v.challenge, v.disc_bits);
        group.measurement_time(Duration::from_secs(10));
        group.bench_with_input(
            BenchmarkId::new("1024bit_depth0", format!("{}iters", v.iters)),
            &(&d, &v.input, &v.proof, v.iters),
            |b, (d, x, p, iters)| {
                b.iter(|| {
                    let result = check_proof_of_time_n_wesolowski(d, x, p, *iters, 0);
                    assert!(result);
                });
            },
        );
    }

    // depth-2: three Wesolowski segments, ~130M iterations, 1024-bit disc
    if let Some(v) = vecs.iter().find(|v| v.depth == 2) {
        let d2 = create_discriminant(&v.challenge, v.disc_bits);
        group.bench_with_input(
            BenchmarkId::new("1024bit_depth2", format!("{}iters", v.iters)),
            &(&d2, &v.input, &v.proof, v.iters),
            |b, (d, x, p, iters)| {
                b.iter(|| {
                    let result = check_proof_of_time_n_wesolowski(d, x, p, *iters, 2);
                    assert!(result);
                });
            },
        );
    }

    // depth-5: six Wesolowski segments
    if let Some(v) = vecs.iter().find(|v| v.depth == 5) {
        let d5 = create_discriminant(&v.challenge, v.disc_bits);
        group.bench_with_input(
            BenchmarkId::new("1024bit_depth5", format!("{}iters", v.iters)),
            &(&d5, &v.input, &v.proof, v.iters),
            |b, (d, x, p, iters)| {
                b.iter(|| {
                    let result = check_proof_of_time_n_wesolowski(d, x, p, *iters, 5);
                    assert!(result);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_verify);
criterion_main!(benches);
