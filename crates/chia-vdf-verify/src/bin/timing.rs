use std::time::Instant;

fn time_fn<F: Fn()>(name: &str, n: u32, f: F) {
    for _ in 0..3 {
        f();
    }
    let t0 = Instant::now();
    for _ in 0..n {
        f();
    }
    let elapsed = t0.elapsed();
    println!(
        "{:<45} {:>8.1} µs/call  (n={})",
        name,
        elapsed.as_micros() as f64 / n as f64,
        n
    );
}

fn main() {
    use chia_vdf_verify::*;
    use malachite_nz::integer::Integer;

    fn hex_decode(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    let d = -integer::from_bytes_be(&hex_decode("d0cb181074454b32a0e0fc5e65a1d7625ea43756eaa8de13a9c750c79f7aa60151f065cd5775516159c28713c1e74ced6520f8f5c55129f32f865b28cf7fe8e7"));
    let x_s = hex_decode("08000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");
    let p_blob = hex_decode("020020417eb39c4e14954a817af644fc13d086c26dddab8afea12415b5e685f7883f5740ba01cb75220081c8aba7854cbd52010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");

    let x = proof_common::deserialize_form(&d, &x_s).unwrap();
    let y = proof_common::deserialize_form(&d, &p_blob[..bqfc::BQFC_FORM_SIZE]).unwrap();
    let mut xm = x.clone();
    let mut ym = y.clone();
    let b = proof_common::get_b(&d, &mut xm, &mut ym);
    let l = form::Form::compute_l(&d);
    let r = proof_common::fast_pow(100, &b);

    println!("=== Timing breakdown (512-bit disc, iters=100) ===");
    println!(
        "B bits = {}, r bits = {}",
        integer::num_bits(&b),
        integer::num_bits(&r)
    );

    time_fn("full verify (iters=100)", 300, || {
        assert!(verifier::check_proof_of_time_n_wesolowski(
            &d, &x_s, &p_blob, 100, 0
        ));
    });

    time_fn("get_b (2x hash_prime + serialize)", 1000, || {
        let mut xm = x.clone();
        let mut ym = y.clone();
        std::hint::black_box(proof_common::get_b(&d, &mut xm, &mut ym));
    });

    let proof = proof_common::deserialize_form(&d, &p_blob[bqfc::BQFC_FORM_SIZE..]).unwrap();
    time_fn("fast_pow_form(proof, B=264-bit)", 300, || {
        std::hint::black_box(proof_common::fast_pow_form_nucomp(&proof, &d, &b, &l));
    });

    time_fn("fast_pow_form(x, r=small)", 10000, || {
        std::hint::black_box(proof_common::fast_pow_form_nucomp(&x, &d, &r, &l));
    });

    time_fn("single nudupl + reduce", 20000, || {
        let f = nucomp::nudupl(&x, &d, &l);
        let mut f = f;
        reducer::reduce(&mut f);
        std::hint::black_box(f);
    });

    time_fn("single nucomp(x,y) + reduce", 20000, || {
        let mut f = nucomp::nucomp(&x, &y, &d, &l);
        reducer::reduce(&mut f);
        std::hint::black_box(f);
    });

    time_fn("reducer::reduce (no-op, already reduced)", 100000, || {
        let mut f = x.clone();
        reducer::reduce(&mut f);
        std::hint::black_box(f);
    });

    let d_abs = Integer::from(d.unsigned_abs_ref().clone());
    time_fn("xgcd_partial (512-bit r2, 264-bit r1)", 20000, || {
        let mut co2 = Integer::from(0i32);
        let mut co1 = Integer::from(0i32);
        let mut r2 = d_abs.clone();
        let mut r1 = b.clone();
        xgcd_partial::xgcd_partial(&mut co2, &mut co1, &mut r2, &mut r1, &l);
        std::hint::black_box((co2, co1));
    });

    time_fn("get_si_2exp (512-bit)", 500000, || {
        std::hint::black_box(integer::get_si_2exp(&d));
    });

    time_fn("hash_prime (264 bits)", 200, || {
        std::hint::black_box(primetest::hash_prime(b"timing_seed_12345", 264, &[263]));
    });

    time_fn("Integer mul (512x512 -> 1024)", 500000, || {
        let r = &d * &d;
        std::hint::black_box(r);
    });

    let half_d = &d >> 1u64;
    let half_d_abs = Integer::from(half_d.unsigned_abs_ref().clone());

    time_fn("fast_extended_gcd malachite (512-bit)", 50000, || {
        let r = chia_vdf_verify::integer::fast_extended_gcd(&d_abs, &b);
        std::hint::black_box(r);
    });

    time_fn("fast_extended_gcd malachite (256-bit)", 50000, || {
        let r = chia_vdf_verify::integer::fast_extended_gcd(&half_d_abs, &b);
        std::hint::black_box(r);
    });

    time_fn("fast_gcd_coeff_b malachite (256-bit)", 50000, || {
        let r = chia_vdf_verify::integer::fast_gcd_coeff_b(&half_d_abs, &b);
        std::hint::black_box(r);
    });
}
