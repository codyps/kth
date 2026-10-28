use core::hint::black_box;
use criterion::{criterion_group, BatchSize, Criterion};
use rand::prelude::*;

#[path = "median_selectors.rs"]
mod median_selectors;

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

fn selection_medians<T: Ord + Copy + core::fmt::Debug>(
    c: &mut Criterion,
    prefix: &str,
    input: &[T],
) {
    macro_rules! register {
        ($name:literal, $values:ident, $index:ident, $select:block) => {
            bench_median(c, &format!("{prefix}/{}", $name), input, |$values| {
                let $index = $values.len() / 2;
                $select;
                $values[$index]
            });
        };
    }
    median_selectors::for_each!(register, values, index);
}

fn median_u8(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(42);
    let input: Vec<u8> = (0..LEN).map(|_| rng.random()).collect();
    selection_medians(c, "median-only/u8", &input);
    // This histogram-based API leaves its input unpartitioned.
    bench_median(c, "median-only/u8/medians", &input, |v| {
        medians::medianu8(v).unwrap().0
    });
}

fn median_u64(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(42);
    let input: Vec<u64> = (0..LEN).map(|_| rng.random()).collect();
    selection_medians(c, "median-only/u64", &input);
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
