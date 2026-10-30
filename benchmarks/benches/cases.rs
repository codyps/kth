use core::hint::black_box;
use criterion::{measurement::WallTime, BatchSize, BenchmarkGroup, BenchmarkId};
use rand::prelude::*;

pub fn partition_cases(n: usize) -> [(&'static str, Vec<u8>); 3] {
    let mut rng = StdRng::seed_from_u64(42);
    let mut random = vec![0u8; n];
    rng.fill(&mut random[..]);
    let mut trailing = vec![0u8; n];
    trailing[n - 1] = 1;
    let mut leading = vec![0u8; n];
    leading[0] = 1;
    [
        ("random-u8", random),
        ("trailing-one", trailing),
        ("leading-one", leading),
    ]
}

pub fn random_u64(n: usize) -> Vec<u64> {
    let mut rng = StdRng::seed_from_u64(42);
    (0..n).map(|_| rng.random()).collect()
}

pub fn validate_partition<T: Ord + Clone + core::fmt::Debug>(
    input: &[T],
    i: usize,
    select: impl Fn(&mut [T], usize),
) {
    let mut expected = input.to_vec();
    expected.sort_unstable();
    let mut actual = input.to_vec();
    select(&mut actual, i);
    assert_eq!(actual[i], expected[i]);
    assert!(actual[..i].iter().all(|x| x <= &actual[i]));
    assert!(actual[i + 1..].iter().all(|x| x >= &actual[i]));
    actual.sort_unstable();
    assert_eq!(actual, expected);
}

pub fn bench_partition_inputs<T: Ord + Clone + core::fmt::Debug>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    algorithm: &str,
    inputs: &[impl AsRef<[T]>],
    scaling: bool,
    select: impl Fn(&mut [T], usize),
) {
    bench_partition_inputs_at(
        group,
        algorithm,
        inputs,
        scaling,
        inputs[0].as_ref().len() / 2,
        select,
    );
}

pub fn bench_partition_inputs_at<T: Ord + Clone + core::fmt::Debug>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    algorithm: &str,
    inputs: &[impl AsRef<[T]>],
    scaling: bool,
    k: usize,
    select: impl Fn(&mut [T], usize),
) {
    let id = if scaling {
        BenchmarkId::new(algorithm, inputs[0].as_ref().len())
    } else {
        BenchmarkId::from_parameter(algorithm)
    };
    let mut next = 0;
    let mut validated = false;
    group.bench_function(id, |b| {
        if !validated {
            for input in inputs {
                validate_partition(input.as_ref(), k, &select);
            }
            validated = true;
        }
        b.iter_batched_ref(
            || {
                let values = inputs[next % inputs.len()].as_ref().to_vec();
                next += 1;
                values
            },
            |v| {
                select(v, k);
                black_box(v);
            },
            if core::mem::size_of_val(inputs[0].as_ref()) >= 1 << 20 {
                BatchSize::PerIteration
            } else {
                BatchSize::LargeInput
            },
        )
    });
}

pub fn bench_median<T: Ord + Clone + core::fmt::Debug>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    algorithm: &str,
    input: &[T],
    scaling: bool,
    median: impl Fn(&mut [T]) -> T,
) {
    let mut sorted = input.to_vec();
    sorted.sort_unstable();
    assert_eq!(median(&mut input.to_vec()), sorted[input.len() / 2]);
    let id = if scaling {
        BenchmarkId::new(algorithm, input.len())
    } else {
        BenchmarkId::from_parameter(algorithm)
    };
    group.bench_function(id, |b| {
        b.iter_batched_ref(
            || input.to_vec(),
            |v| black_box(median(v)),
            BatchSize::LargeInput,
        )
    });
}

pub fn partitions<T: Ord + Copy + core::fmt::Debug>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    input: &[T],
    scaling: bool,
) {
    partition_inputs(group, &[input], scaling);
}

pub fn byte_partitions(group: &mut BenchmarkGroup<'_, WallTime>, input: &[u8], scaling: bool) {
    partitions(group, input, scaling);
    bench_partition_inputs(group, "kth-u8", &[input], scaling, kth::partition_u8_by_kth);
}

pub fn partition_inputs<T: Ord + Copy + core::fmt::Debug>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    inputs: &[impl AsRef<[T]>],
    scaling: bool,
) {
    partition_inputs_at(group, inputs, scaling, inputs[0].as_ref().len() / 2);
}

pub fn partition_inputs_at<T: Ord + Copy + core::fmt::Debug>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    inputs: &[impl AsRef<[T]>],
    scaling: bool,
    k: usize,
) {
    macro_rules! register {
        ($name:literal, $values:ident, $index:ident, $select:block) => {
            bench_partition_inputs_at(group, $name, inputs, scaling, k, |$values, $index| {
                let _ = $index;
                $select
            });
        };
    }
    super::selectors::for_each!(register, values, index);
}

pub fn owned_partitions<T: Ord + Clone + core::fmt::Debug>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    inputs: &[impl AsRef<[T]>],
    scaling: bool,
) {
    owned_partitions_at(group, inputs, scaling, inputs[0].as_ref().len() / 2);
}

pub fn owned_partitions_at<T: Ord + Clone + core::fmt::Debug>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    inputs: &[impl AsRef<[T]>],
    scaling: bool,
    k: usize,
) {
    macro_rules! register {
        ($name:literal, $values:ident, $index:ident, $select:block) => {
            bench_partition_inputs_at(group, $name, inputs, scaling, k, |$values, $index| {
                let _ = $index;
                $select
            });
        };
    }
    super::selectors::for_each_generic!(register, values, index);
}

pub fn medians<T: Ord + Copy + core::fmt::Debug>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    input: &[T],
    scaling: bool,
    specialized: impl Fn(&mut [T]) -> T,
) {
    macro_rules! register {
        ($name:literal, $values:ident, $index:ident, $select:block) => {
            bench_median(group, $name, input, scaling, |$values| {
                let $index = $values.len() / 2;
                $select;
                $values[$index]
            });
        };
    }
    super::selectors::for_each!(register, values, index);
    // medians' histogram-based u8 API retrieves a value without partitioning.
    bench_median(group, "medians", input, scaling, specialized);
}

pub fn median_cases(c: &mut criterion::Criterion, suite: &str, n: usize, scaling: bool) {
    let mut rng = StdRng::seed_from_u64(42);
    let input: Vec<u8> = (0..n).map(|_| rng.random()).collect();
    let mut group = c.benchmark_group(format!("{suite}/u8"));
    medians(&mut group, &input, scaling, |v| {
        medians::medianu8(v).unwrap().0
    });
    bench_median(&mut group, "kth-u8", &input, scaling, |v| {
        let k = v.len() / 2;
        kth::partition_u8_by_kth(v, k);
        v[k]
    });
    group.finish();
    let mut group = c.benchmark_group(format!("{suite}/u64"));
    medians(
        &mut group,
        &random_u64(n),
        scaling,
        |v| match medians::medianu64(v).unwrap() {
            medians::Medians::Odd(value) => *value,
            medians::Medians::Even(_) => unreachable!("odd input length"),
        },
    );
    group.finish();
}
