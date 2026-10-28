use core::hint::black_box;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use rand::prelude::*;

#[path = "../../../benches/sizes.rs"]
mod sizes;

fn bench_median<T: Ord + Copy + core::fmt::Debug>(
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
            BatchSize::LargeInput,
        )
    });
}

fn comparison<T: Ord + Copy + core::fmt::Debug>(
    c: &mut Criterion,
    prefix: &str,
    suffix: &str,
    input: &[T],
    specialized: impl Fn(&mut [T]) -> T,
) {
    bench_median(c, &format!("{prefix}/kth{suffix}"), input, |v| {
        let i = v.len() / 2;
        kth::partition_by_kth(v, i);
        v[i]
    });
    bench_median(c, &format!("{prefix}/std{suffix}"), input, |v| {
        let i = v.len() / 2;
        *v.select_nth_unstable(i).1
    });
    bench_median(c, &format!("{prefix}/turboselect{suffix}"), input, |v| {
        let i = v.len() / 2;
        *turboselect::select_nth_unstable(v, i).1
    });
    bench_median(c, &format!("{prefix}/medians{suffix}"), input, specialized);
}

fn run(c: &mut Criterion, n: usize, suite: &str, scaling: bool) {
    let suffix = if scaling {
        format!("/{n}")
    } else {
        String::new()
    };
    let mut rng = StdRng::seed_from_u64(42);
    let input: Vec<u8> = (0..n).map(|_| rng.random()).collect();
    comparison(c, &format!("{suite}/u8"), &suffix, &input, |v| {
        medians::medianu8(v).unwrap().0
    });
    let mut rng = StdRng::seed_from_u64(42);
    let input: Vec<u64> = (0..n).map(|_| rng.random()).collect();
    comparison(
        c,
        &format!("{suite}/u64"),
        &suffix,
        &input,
        |v| match medians::medianu64(v).unwrap() {
            medians::Medians::Odd(value) => *value,
            medians::Medians::Even(_) => unreachable!("odd input length"),
        },
    );
}

fn fixed(c: &mut Criterion) {
    run(c, 8193, "median-only-experimental", false);
}

fn scaling(c: &mut Criterion) {
    let sizes: std::collections::BTreeSet<_> = sizes::sizes().into_iter().map(|n| n | 1).collect();
    for n in sizes {
        run(c, n, "scaling-median-experimental", true);
    }
}

criterion_group!(medians, fixed, scaling);
criterion_main!(medians);
