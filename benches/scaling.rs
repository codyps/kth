use core::hint::black_box;
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use rand::prelude::*;

mod median_selectors;
mod scaling_cases;

fn partitions(c: &mut Criterion) {
    for n in scaling_cases::sizes() {
        for (case, input) in scaling_cases::partition_cases(n) {
            let mut group = c.benchmark_group(format!("scaling-partition/{case}"));
            scaling_cases::bench_partition(&mut group, "kth", &input, kth::partition_by_kth);
            scaling_cases::bench_partition(&mut group, "std", &input, |v, i| {
                v.select_nth_unstable(i);
            });
            scaling_cases::bench_partition(&mut group, "order-stat", &input, |v, i| {
                order_stat::kth(v, i);
            });
            scaling_cases::bench_partition(&mut group, "pdqselect", &input, pdqselect::select);
            scaling_cases::bench_partition(&mut group, "adqselect", &input, |v, i| {
                adqselect::nth_element(v, i, &mut Ord::cmp);
            });
            scaling_cases::bench_partition(&mut group, "tpt-simd-sort", &input, |v, i| {
                tpt_simd_sort::select_nth(v, i);
            });
            scaling_cases::bench_partition(&mut group, "sort", &input, |v, _| v.sort());
            scaling_cases::bench_partition(&mut group, "sort_unstable", &input, |v, _| {
                v.sort_unstable();
            });
            group.finish();
        }
    }
}

fn bench_median<T: Ord + Clone + core::fmt::Debug>(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    algorithm: &str,
    input: &[T],
    median: impl Fn(&mut [T]) -> T,
) {
    let mut sorted = input.to_vec();
    sorted.sort_unstable();
    assert_eq!(median(&mut input.to_vec()), sorted[input.len() / 2]);
    group.bench_function(BenchmarkId::new(algorithm, input.len()), |b| {
        b.iter_batched_ref(
            || input.to_vec(),
            |v| black_box(median(v)),
            BatchSize::LargeInput,
        )
    });
}

fn selection_medians<T: Ord + Copy + core::fmt::Debug>(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    input: &[T],
) {
    macro_rules! register {
        ($name:literal, $values:ident, $index:ident, $select:block) => {
            bench_median(group, $name, input, |$values| {
                let $index = $values.len() / 2;
                $select;
                $values[$index]
            });
        };
    }
    median_selectors::for_each!(register, values, index);
}

fn medians(c: &mut Criterion) {
    // Keep the actual odd length in each benchmark ID and on the chart's x axis.
    let sizes: std::collections::BTreeSet<_> =
        scaling_cases::sizes().into_iter().map(|n| n | 1).collect();
    for n in sizes {
        let mut rng = StdRng::seed_from_u64(42);
        let input: Vec<u8> = (0..n).map(|_| rng.random()).collect();
        let mut group = c.benchmark_group("scaling-median/u8");
        selection_medians(&mut group, &input);
        bench_median(&mut group, "medians", &input, |v| {
            medians::medianu8(v).unwrap().0
        });
        group.finish();

        let mut rng = StdRng::seed_from_u64(42);
        let input: Vec<u64> = (0..n).map(|_| rng.random()).collect();
        let mut group = c.benchmark_group("scaling-median/u64");
        selection_medians(&mut group, &input);
        bench_median(
            &mut group,
            "medians",
            &input,
            |v| match medians::medianu64(v).unwrap() {
                medians::Medians::Odd(value) => *value,
                medians::Medians::Even(_) => unreachable!("odd input length"),
            },
        );
        group.finish();
    }
}

criterion_group!(scaling, partitions, medians);
criterion_main!(scaling);
