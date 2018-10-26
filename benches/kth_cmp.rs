#[macro_use]
extern crate criterion;

extern crate kth;
extern crate order_stat;
extern crate pdqselect;

extern crate rand;
use rand::prelude::*;

use criterion::Criterion;

fn vec_0x1024_1() -> Vec<u8>
{
    let mut v = Vec::with_capacity(1024);
    for _ in 0..1023 {
        v.push(0);
    }
    v.push(1);
    v
}

fn vec_rand() -> Vec<u8>
{
    let mut v = vec![0u8;2048];
    let mut rng = thread_rng();

    rng.fill(&mut v[..]);
    v
}

fn bench_kth(c: &mut Criterion) {
    c.bench_function("kth [0]*1024+[1]",
        |b| {
            let mut v = vec_0x1024_1();
            b.iter(
                || {
                    let i = v.len() / 2;
                    kth::partition_by_kth(&mut v[..], i);
                }
            )
        }
    );
}

fn bench_order_stat(c: &mut Criterion) {
    c.bench_function("order-stat [0]*1024+[1]",
        |b| {
            let mut v = vec_0x1024_1();
            b.iter(
                || {
                    let i = v.len() / 2;
                    order_stat::kth(&mut v[..], i);
                }
            )
        }
    );
}

fn bench_pdqselect(c: &mut Criterion) {
    c.bench_function("pdqselect [0]*1024+[1]",
        |b| {
            let mut v = vec_0x1024_1();
            b.iter(
                || {
                    let i = v.len() / 2;
                    pdqselect::select(&mut v[..], i);
                }
            )
        }
    );
}

fn bench_rand_kth(c: &mut Criterion) {
    c.bench_function("kth rand",
        |b| {
            let mut v = vec_rand();
            b.iter(
                || {
                    let i = v.len() / 2;
                    kth::partition_by_kth(&mut v[..], i);
                }
            )
        }
    );
}

fn bench_rand_order_stat(c: &mut Criterion) {
    c.bench_function("order-stat rand",
        |b| {
            let mut v = vec_rand();
            b.iter(
                || {
                    let i = v.len() / 2;
                    order_stat::kth(&mut v[..], i);
                }
            )
        }
    );
}

fn bench_rand_pdqselect(c: &mut Criterion) {
    c.bench_function("pdqselect rand",
        |b| {
            let mut v = vec_rand();
            b.iter(
                || {
                    let i = v.len() / 2;
                    pdqselect::select(&mut v[..], i);
                }
            )
        }
    );
}

criterion_group!(rand, bench_rand_kth, bench_rand_order_stat, bench_rand_pdqselect);
criterion_group!(one_one, bench_kth, bench_order_stat, bench_pdqselect);
criterion_main!(one_one, rand);
