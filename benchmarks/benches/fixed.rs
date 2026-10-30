use super::cases;
use core::hint::black_box;
use criterion::{criterion_group, Criterion};
use rand::prelude::*;

fn fixed_partitions(c: &mut Criterion) {
    for (case, n) in [
        ("random-u8", 8192),
        ("trailing-one", 4196),
        ("leading-one", 4196),
    ] {
        let input = cases::partition_cases(n)
            .into_iter()
            .find(|(name, _)| *name == case)
            .unwrap()
            .1;
        let mut group = c.benchmark_group(format!("partition/{case}"));
        cases::byte_partitions(&mut group, &input, false);
        group.finish();
    }
    let mut group = c.benchmark_group("partition/random-u64");
    cases::partitions(&mut group, &cases::random_u64(8192), false);
    group.finish();
}

fn fixed_medians(c: &mut Criterion) {
    cases::median_cases(c, "median-only", 8193, false);
}

// This group deliberately includes RNG cost and is excluded from report comparisons.
fn rng_partitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("rng-partition/random-u8");
    macro_rules! register {
        ($name:literal, $values:ident, $index:ident, $select:block) => {
            group.bench_function($name, |b| {
                let mut rng = StdRng::seed_from_u64(42);
                let mut input = vec![0u8; 8192];
                rng.fill(&mut input[..]);
                cases::validate_partition(&input, input.len() / 2, |$values, $index| {
                    let _ = $index;
                    $select
                });
                b.iter(|| {
                    rng.fill(&mut input[..]);
                    let $values = &mut input[..];
                    let $index = $values.len() / 2;
                    $select;
                    black_box($values[$index]);
                });
            });
        };
    }
    super::selectors::for_each!(register, values, index);
    group.finish();
}

criterion_group!(fixed, fixed_partitions, fixed_medians, rng_partitions);
