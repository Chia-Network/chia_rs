/// Benchmark: num-bigint vs malachite-nz for operations relevant to VDF verification.
///
/// Operations tested:
/// - modpow (used in hash_prime / Miller-Rabin)
/// - extended_gcd (used in nucomp / nudupl)
/// - multiply / divide (used throughout)
///
/// Run with:
///   cargo bench --bench bigint_compare
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use malachite_base::num::arithmetic::traits::{ExtendedGcd, ModPow};
use malachite_base::num::basic::traits::Zero;
use malachite_nz::integer::Integer as MalachiteInteger;
use malachite_nz::natural::Natural;
use num_bigint::BigUint;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn nat_from_be_bytes(bytes: &[u8]) -> Natural {
    let mut n = Natural::ZERO;
    for &b in bytes {
        n <<= 8u64;
        n += Natural::from(b);
    }
    n
}

fn mal_int_from_be_bytes(bytes: &[u8]) -> MalachiteInteger {
    MalachiteInteger::from(nat_from_be_bytes(bytes))
}

fn biguint_from_be_bytes(bytes: &[u8]) -> BigUint {
    BigUint::from_bytes_be(bytes)
}

/// Build (base, exponent, modulus) at `bits` bit-size as pseudo-random byte patterns.
fn make_numbers(
    bits: usize,
) -> (
    BigUint,
    BigUint,
    BigUint,
    Natural,
    Natural,
    Natural,
    MalachiteInteger,
    MalachiteInteger,
) {
    let byte_len = bits / 8;
    let base_bytes: Vec<u8> = (0..byte_len)
        .map(|i| ((i.wrapping_mul(97).wrapping_add(17)) & 0xff) as u8)
        .collect();
    let exp_bytes: Vec<u8> = (0..byte_len)
        .map(|i| ((i.wrapping_mul(131).wrapping_add(37)) & 0xff) as u8)
        .collect();
    let mut mod_bytes: Vec<u8> = (0..byte_len)
        .map(|i| ((i.wrapping_mul(173).wrapping_add(53)) & 0xff) as u8)
        .collect();
    mod_bytes[0] |= 0x80;
    mod_bytes[byte_len - 1] |= 0x01;

    let b_base = biguint_from_be_bytes(&base_bytes);
    let b_exp = biguint_from_be_bytes(&exp_bytes);
    let b_mod = biguint_from_be_bytes(&mod_bytes);
    let b_base_r = &b_base % &b_mod;

    let m_base = nat_from_be_bytes(&base_bytes);
    let m_exp = nat_from_be_bytes(&exp_bytes);
    let m_mod = nat_from_be_bytes(&mod_bytes);
    let m_base_r = &m_base % &m_mod;

    let mi_a = mal_int_from_be_bytes(&base_bytes);
    let mi_b = mal_int_from_be_bytes(&exp_bytes);

    (b_base_r, b_exp, b_mod, m_base_r, m_exp, m_mod, mi_a, mi_b)
}

// ── modpow ─────────────────────────────────────────────────────────────────────

fn bench_modpow(c: &mut Criterion) {
    let mut group = c.benchmark_group("modpow");

    for bits in [256usize, 512, 1024] {
        let (b_base, b_exp, b_mod, m_base, m_exp, m_mod, _, _) = make_numbers(bits);

        group.bench_with_input(BenchmarkId::new("num-bigint", bits), &bits, |b, _| {
            b.iter(|| {
                black_box(b_base.modpow(&b_exp, &b_mod));
            });
        });

        group.bench_with_input(BenchmarkId::new("malachite", bits), &bits, |b, _| {
            b.iter(|| {
                black_box((&m_base).mod_pow(&m_exp, &m_mod));
            });
        });
    }

    group.finish();
}

// ── extended_gcd ──────────────────────────────────────────────────────────────

fn bench_extended_gcd(c: &mut Criterion) {
    let mut group = c.benchmark_group("extended_gcd");

    for bits in [256usize, 512, 1024] {
        let byte_len = bits / 8;
        let a_bytes: Vec<u8> = (0..byte_len)
            .map(|i| ((i.wrapping_mul(199).wrapping_add(7)) & 0xff) as u8)
            .collect();
        let mut b_bytes: Vec<u8> = (0..byte_len)
            .map(|i| ((i.wrapping_mul(251).wrapping_add(13)) & 0xff) as u8)
            .collect();
        b_bytes[0] |= 0x80;
        b_bytes[byte_len - 1] |= 0x01;

        let mal_a = mal_int_from_be_bytes(&a_bytes);
        let mal_b = mal_int_from_be_bytes(&b_bytes);

        // num-bigint fast_extended_gcd now takes malachite Integer (since the lib is ported)
        group.bench_with_input(
            BenchmarkId::new("malachite (built-in xgcd)", bits),
            &bits,
            |b, _| {
                b.iter(|| {
                    black_box(chia_vdf_verify::integer::fast_extended_gcd(&mal_a, &mal_b));
                });
            },
        );

        let mal_a2 = nat_from_be_bytes(&a_bytes);
        let mal_b2 = nat_from_be_bytes(&b_bytes);

        group.bench_with_input(
            BenchmarkId::new("malachite (Natural xgcd)", bits),
            &bits,
            |b, _| {
                b.iter(|| {
                    black_box(mal_a2.clone().extended_gcd(mal_b2.clone()));
                });
            },
        );
    }

    group.finish();
}

// ── multiply ──────────────────────────────────────────────────────────────────

fn bench_multiply(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiply");

    for bits in [256usize, 512, 1024] {
        let (b_base, b_exp, _b_mod, m_base, m_exp, _m_mod, _, _) = make_numbers(bits);

        group.bench_with_input(BenchmarkId::new("num-bigint", bits), &bits, |b, _| {
            b.iter(|| {
                black_box(&b_base * &b_exp);
            });
        });

        group.bench_with_input(BenchmarkId::new("malachite", bits), &bits, |b, _| {
            b.iter(|| {
                black_box(&m_base * &m_exp);
            });
        });
    }

    group.finish();
}

// ── divide ────────────────────────────────────────────────────────────────────

fn bench_divide(c: &mut Criterion) {
    let mut group = c.benchmark_group("divide");

    for bits in [256usize, 512, 1024] {
        let (b_base, b_exp, _b_mod, m_base, m_exp, _m_mod, _, _) = make_numbers(bits);
        let b_dividend = &b_base * &b_exp;
        let m_dividend = &m_base * &m_exp;

        group.bench_with_input(BenchmarkId::new("num-bigint", bits), &bits, |b, _| {
            b.iter(|| {
                black_box(&b_dividend / &b_exp);
            });
        });

        group.bench_with_input(BenchmarkId::new("malachite", bits), &bits, |b, _| {
            b.iter(|| {
                black_box(&m_dividend / &m_exp);
            });
        });
    }

    group.finish();
}

// ── full VDF verify ───────────────────────────────────────────────────────────

fn bench_full_verify(c: &mut Criterion) {
    use chia_vdf_verify::integer::from_bytes_be;
    use chia_vdf_verify::verifier::check_proof_of_time_n_wesolowski;

    fn hex_decode(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    const D1_HEX: &str = "d0cb181074454b32a0e0fc5e65a1d7625ea43756eaa8de13a9c750c79f7aa60151f065cd5775516159c28713c1e74ced6520f8f5c55129f32f865b28cf7fe8e7";
    const X_HEX: &str  = "08000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
    const P1_HEX: &str = "020020417eb39c4e14954a817af644fc13d086c26dddab8afea12415b5e685f7883f5740ba01cb75220081c8aba7854cbd52010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

    let d1: MalachiteInteger = -from_bytes_be(&hex_decode(D1_HEX));
    let x_s = hex_decode(X_HEX);
    let p1 = hex_decode(P1_HEX);

    let mut group = c.benchmark_group("full_verify");
    group.bench_function("malachite/iters=100", |b| {
        b.iter(|| {
            let result = check_proof_of_time_n_wesolowski(&d1, &x_s, &p1, 100, 0);
            assert!(result);
        });
    });
    group.finish();
}

// ── entry point ───────────────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_modpow,
    bench_extended_gcd,
    bench_multiply,
    bench_divide,
    bench_full_verify,
);
criterion_main!(benches);
