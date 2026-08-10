/// Real-world mainnet VDF verification benchmark.
///
/// Uses pre-extracted reward-chain IP proofs from `proofs.json` (the same
/// corpus previously exercised by the Python `vdf_cpp_vs_rust.py` script).
/// Regenerating the fixture: `python benches/extract_proofs.py`.
///
/// Run with:
///   cargo bench --bench mainnet_proofs
use chia_vdf_verify::discriminant::create_discriminant;
use chia_vdf_verify::verifier::check_proof_of_time_n_wesolowski;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use malachite_nz::integer::Integer;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

const DISC_BITS: usize = 1024;
const IDENTITY: [u8; 100] = {
    let mut id = [0u8; 100];
    id[0] = 0x08;
    id
};

#[derive(Deserialize)]
struct ProofRecord {
    challenge: String,
    #[serde(default)]
    input_el: Option<String>,
    proof_blob: String,
    iters: u64,
    witness_type: u64,
}

struct Case {
    disc: Integer,
    input_el: Vec<u8>,
    proof_blob: Vec<u8>,
    iters: u64,
    witness_type: u64,
}

fn load_cases() -> Vec<Case> {
    let raw: Vec<ProofRecord> =
        serde_json::from_str(include_str!("proofs.json")).expect("parse proofs.json");

    let mut discs: HashMap<Vec<u8>, Integer> = HashMap::new();
    raw.into_iter()
        .map(|r| {
            let challenge = hex::decode(&r.challenge).expect("challenge hex");
            let disc = discs
                .entry(challenge.clone())
                .or_insert_with(|| create_discriminant(&challenge, DISC_BITS))
                .clone();
            let input_el = r
                .input_el
                .as_deref()
                .map(|s| hex::decode(s).expect("input_el hex"))
                .unwrap_or_else(|| IDENTITY.to_vec());
            Case {
                disc,
                input_el,
                proof_blob: hex::decode(&r.proof_blob).expect("proof_blob hex"),
                iters: r.iters,
                witness_type: r.witness_type,
            }
        })
        .collect()
}

fn verify_case(case: &Case) -> bool {
    check_proof_of_time_n_wesolowski(
        &case.disc,
        &case.input_el,
        &case.proof_blob,
        case.iters,
        case.witness_type,
    )
}

fn bench_mainnet_proofs(c: &mut Criterion) {
    let cases = load_cases();
    assert!(
        !cases.is_empty(),
        "proofs.json must contain at least one proof"
    );
    assert!(
        cases.iter().all(verify_case),
        "all fixture proofs must verify before benchmarking"
    );

    let mut group = c.benchmark_group("mainnet_proofs");
    // Full corpus (~100 proofs) is expensive; keep sample count modest for CI.
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(30));

    group.bench_function(
        BenchmarkId::new("all", format!("{}proofs", cases.len())),
        |b| {
            b.iter(|| {
                for case in &cases {
                    assert!(verify_case(case));
                }
            });
        },
    );

    // One representative proof per common witness type / depth.
    for depth in [0u64, 1, 2, 3, 4, 5] {
        if let Some(case) = cases.iter().find(|c| c.witness_type == depth) {
            group.bench_with_input(BenchmarkId::new("witness_type", depth), case, |b, case| {
                b.iter(|| {
                    assert!(verify_case(case));
                });
            });
        }
    }

    group.finish();
}

criterion_group!(benches, bench_mainnet_proofs);
criterion_main!(benches);
