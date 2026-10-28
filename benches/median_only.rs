use core::hint::black_box;
use criterion::{criterion_group, BatchSize, Criterion};
use rand::prelude::*;

// Odd lengths give every implementation the same single-median result.
const LEN: usize = 8193;

fn bench_median<T: Ord + Clone + core::fmt::Debug>(
    c: &mut Criterion,
    name: &str,
    input: &[T],
    median: impl Fn(&mut [T]) -> T,
) {
    let mut sorted = input.to_vec();
    sorted.sort_unstable();
    assert_eq!(median(&mut input.to_vec()), sorted[input.len() / 2]);
    c.bench_function(name, |b| {
        b.iter_batched_ref(
            || input.to_vec(),
            |v| black_box(median(v)),
            BatchSize::SmallInput,
        )
    });
}

fn median_u8(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(42);
    let input: Vec<u8> = (0..LEN).map(|_| rng.random()).collect();
    bench_median(c, "median-only/u8/kth", &input, |v| {
        let i = v.len() / 2;
        kth::partition_by_kth(v, i);
        v[i]
    });
    bench_median(c, "median-only/u8/std", &input, |v| {
        let i = v.len() / 2;
        *v.select_nth_unstable(i).1
    });
    // This histogram-based API leaves its input unpartitioned.
    bench_median(c, "median-only/u8/medians", &input, |v| {
        medians::medianu8(v).unwrap().0
    });
}

fn median_u64(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(42);
    let input: Vec<u64> = (0..LEN).map(|_| rng.random()).collect();
    bench_median(c, "median-only/u64/kth", &input, |v| {
        let i = v.len() / 2;
        kth::partition_by_kth(v, i);
        v[i]
    });
    bench_median(c, "median-only/u64/std", &input, |v| {
        let i = v.len() / 2;
        *v.select_nth_unstable(i).1
    });
    bench_median(
        c,
        "median-only/u64/medians",
        &input,
        |v| match medians::medianu64(v).unwrap() {
            medians::Medians::Odd(value) => *value,
            medians::Medians::Even(_) => unreachable!("the dataset has odd length"),
        },
    );
}

criterion_group!(median_only, median_u8, median_u64);
