#![allow(clippy::many_single_char_names)]

use chia_vdf_verify::discriminant::create_discriminant;
use chia_vdf_verify::form::Form;
use chia_vdf_verify::nucomp::{nucomp, nudupl};
use chia_vdf_verify::reducer::reduce;
use chia_vdf_verify::xgcd_partial::xgcd_partial;
use criterion::{Criterion, criterion_group, criterion_main};
use malachite_base::num::basic::traits::Zero;
use malachite_nz::integer::Integer;

fn bench_micro(c: &mut Criterion) {
    let d = create_discriminant(b"bench_micro_seed", 1024);
    let l = Form::compute_l(&d);
    let generator = Form::generator(&d);

    let mut f = generator.clone();
    for _ in 0..20 {
        f = nudupl(&f, &d, &l);
        reduce(&mut f);
    }
    let g = f.clone();

    c.bench_function("nudupl_1024", |b| {
        b.iter(|| {
            let r = nudupl(&f, &d, &l);
            std::hint::black_box(r);
        });
    });

    c.bench_function("reduce_1024", |b| {
        let unreduced = nudupl(&f, &d, &l);
        b.iter(|| {
            let mut copy = unreduced.clone();
            reduce(&mut copy);
            std::hint::black_box(copy);
        });
    });

    c.bench_function("nucomp_1024", |b| {
        b.iter(|| {
            let r = nucomp(&f, &g, &d, &l);
            std::hint::black_box(r);
        });
    });

    c.bench_function("xgcd_partial_1024", |b| {
        b.iter(|| {
            let mut co2 = Integer::ZERO;
            let mut co1 = Integer::ZERO;
            let mut r2 = f.a.clone();
            let mut r1 = f.b.clone();
            xgcd_partial(&mut co2, &mut co1, &mut r2, &mut r1, &l);
            std::hint::black_box((&co2, &co1, &r2, &r1));
        });
    });
}

criterion_group!(benches, bench_micro);
criterion_main!(benches);
