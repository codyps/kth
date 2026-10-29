use super::{cases, inputs, sizes};
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
        let random = inputs::random_bank(n);
        for case in inputs::string_cases(false) {
            let owned = inputs::strings(case, &random);
            let mut group = c.benchmark_group(format!("scaling-partition/string-{case}"));
            cases::owned_partitions(&mut group, &owned, true);
            group.finish();
            let borrowed: Vec<Vec<&str>> = owned
                .iter()
                .map(|v| v.iter().map(String::as_str).collect())
                .collect();
            let mut group = c.benchmark_group(format!("scaling-partition/str-{case}"));
            cases::partition_inputs(&mut group, &borrowed, true);
            group.finish();
        }
        let mut group = c.benchmark_group("scaling-partition/random-wide64");
        cases::partition_inputs(&mut group, &inputs::wide(&random), true);
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
