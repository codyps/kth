use super::{cases, sizes};
use criterion::{criterion_group, Criterion};

fn partitions(c: &mut Criterion) {
    for n in sizes::sizes() {
        for (case, input) in cases::partition_cases(n) {
            let mut group = c.benchmark_group(format!("scaling-partition/{case}"));
            cases::partitions(&mut group, &input, true);
            group.finish();
        }
        let mut group = c.benchmark_group("scaling-partition/random-u64");
        cases::partitions(&mut group, &cases::random_u64(n), true);
        group.finish();
    }
}

fn medians(c: &mut Criterion) {
    let sizes: std::collections::BTreeSet<_> = sizes::sizes().into_iter().map(|n| n | 1).collect();
    for n in sizes {
        cases::median_cases(c, "scaling-median", n, true);
    }
}

criterion_group!(scaling, partitions, medians);
