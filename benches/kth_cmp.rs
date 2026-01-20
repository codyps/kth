use rand::prelude::*;

use core::hint::black_box;
use criterion::{criterion_group, Criterion};

macro_rules! bench_fixed {
    ($c:ident, $name:literal, $v:expr) => {
        paste::paste! {
            fn [< bench_kth_ $c >](c: &mut Criterion) {
                c.bench_function(concat!("kth ", $name), |b| {
                    let mut v = $v;
                    b.iter(|| {
                        let i = v.len() / 2;
                        kth::partition_by_kth(&mut v[..], i);
                        black_box(v[i]);
                    })
                });
            }

            fn [< bench_order_stat_ $c >](c: &mut Criterion) {
                c.bench_function(concat!("order-stat ", $name), |b| {
                    let mut v = $v;
                    b.iter(|| {
                        let i = v.len() / 2;
                        order_stat::kth(&mut v[..], i);
                        black_box(v[i]);
                    })
                });
            }

            fn [< bench_pdqselect_ $c >](c: &mut Criterion) {
                c.bench_function(concat!("pdqselect ", $name), |b| {
                    let mut v = $v;
                    b.iter(|| {
                        let i = v.len() / 2;
                        pdqselect::select(&mut v[..], i);
                        black_box(v[i]);
                    })
                });
            }

            fn [< bench_std_ $c >](c: &mut Criterion) {
                c.bench_function(concat!("std ", $name), |b| {
                    let mut v = $v;
                    b.iter(|| {
                        let i = v.len() / 2;
                        v.select_nth_unstable(i);
                        black_box(v[i]);
                    })
                });
            }

            fn [< bench_sort_ $c >](c: &mut Criterion) {
                c.bench_function(concat!("sort ", $name), |b| {
                    let mut v = $v;
                    b.iter(|| {
                        v.sort();
                        black_box(v[v.len() / 2]);
                    })
                });
            }

            fn [< bench_sort_unstable_ $c >](c: &mut Criterion) {
                c.bench_function(concat!("sort_unstable ", $name), |b| {
                    let mut v = $v;
                    b.iter(|| {
                        v.sort_unstable();
                        black_box(v[v.len() / 2]);
                    })
                });
            }

            criterion_group!(
                $c,
                [< bench_kth_ $c >],
                [< bench_order_stat_ $c >],
                [< bench_pdqselect_ $c >],
                [< bench_std_ $c >],
                [< bench_sort_ $c >],
                [< bench_sort_unstable_ $c >],
            );
        }
    };
}

fn vec_0x4195_1() -> Vec<u8> {
    let mut v = Vec::with_capacity(4196);
    for _ in 0..4195 {
        v.push(0);
    }
    v.push(1);
    v
}

fn vec_1_0x4195() -> Vec<u8> {
    let mut v = Vec::with_capacity(4196);

    v.push(1);
    for _ in 0..4195 {
        v.push(0);
    }
    v
}

fn vec_rand_fixed() -> Vec<u8> {
    let mut v = vec![0u8; RAND_LEN];
    let mut rng = StdRng::seed_from_u64(42);
    rng.fill(&mut v[..]);
    v
}

bench_fixed!(ends_with_1, "[0]*4195+[1]", vec_0x4195_1());
bench_fixed!(starts_with_1, "[1]+[0]*4195", vec_1_0x4195());
bench_fixed!(rand_fixed, "rand fixed", vec_rand_fixed());

const RAND_LEN: usize = 8192;

fn bench_rand_kth(c: &mut Criterion) {
    c.bench_function("kth rand", |b| {
        let mut rng = rand::rng();
        let mut v = vec![0u8; RAND_LEN];
        b.iter(|| {
            rng.fill_bytes(&mut v);
            let i = v.len() / 2;
            kth::partition_by_kth(&mut v[..], i);
            black_box(v[i]);
        })
    });
}

fn bench_rand_order_stat(c: &mut Criterion) {
    c.bench_function("order-stat rand", |b| {
        let mut rng = rand::rng();
        let mut v = vec![0u8; RAND_LEN];
        b.iter(|| {
            rng.fill_bytes(&mut v);
            let i = v.len() / 2;
            order_stat::kth(&mut v[..], i);
            black_box(v[i]);
        })
    });
}

fn bench_rand_pdqselect(c: &mut Criterion) {
    c.bench_function("pdqselect rand", |b| {
        let mut rng = rand::rng();
        let mut v = vec![0u8; RAND_LEN];
        b.iter(|| {
            rng.fill_bytes(&mut v);
            let i = v.len() / 2;
            pdqselect::select(&mut v[..], i);
            black_box(v[i]);
        })
    });
}

fn bench_rand_std(c: &mut Criterion) {
    c.bench_function("std rand", |b| {
        let mut rng = rand::rng();
        let mut v = vec![0u8; 8192];
        b.iter(|| {
            rng.fill_bytes(&mut v);
            let i = v.len() / 2;
            v.select_nth_unstable(i);
            black_box(v[i]);
        })
    });
}

fn bench_rand_sort(c: &mut Criterion) {
    c.bench_function("sort rand", |b| {
        let mut rng = rand::rng();
        let mut v = vec![0u8; RAND_LEN];
        b.iter(|| {
            rng.fill_bytes(&mut v);
            v.sort();
            black_box(v[v.len() / 2]);
        })
    });
}

fn bench_rand_sort_unstable(c: &mut Criterion) {
    c.bench_function("sort_unstable rand", |b| {
        let mut rng = rand::rng();
        let mut v = vec![0u8; RAND_LEN];
        b.iter(|| {
            rng.fill_bytes(&mut v);
            v.sort_unstable();
            black_box(v[v.len() / 2]);
        })
    });
}
criterion_group!(
    rand,
    bench_rand_kth,
    bench_rand_order_stat,
    bench_rand_pdqselect,
    bench_rand_std,
    bench_rand_sort,
    bench_rand_sort_unstable
);

/*
criterion_group!(
    rand_fixed,
    bench_rand_fixed_kth,
    bench_rand_fixed_order_stat,
    bench_rand_fixed_pdqselect,
    bench_rand_fixed_std,
    bench_rand_fixed_sort,
    bench_rand_fixed_sort_unstable
);
criterion_group!(
    one_one,
    bench_kth,
    bench_order_stat,
    bench_pdqselect,
    bench_std,
    bench_sort,
    bench_sort_unstable
);
*/
