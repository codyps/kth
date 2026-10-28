//! Compare the public selector with its original algorithm and Rust's selector.
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::hint::black_box;

mod sizes;

fn original<T: Ord>(values: &mut [T], k: usize) {
    kth::quickselect::quickselect(kth::quickselect::repeated_step3, values, k);
}

fn standard<T: Ord>(values: &mut [T], k: usize) {
    values.select_nth_unstable(k);
}

fn bench<T: Ord + Clone + std::fmt::Debug>(
    c: &mut Criterion,
    name: &str,
    inputs: &[Vec<T>],
    rank: &str,
    k: usize,
) {
    let mut group = c.benchmark_group(format!("selection/{name}/{rank}"));
    macro_rules! register {
        ($name:literal, $select:path) => {{
            for input in inputs {
                let mut expected = input.clone();
                expected.sort_unstable();
                let mut actual = input.clone();
                $select(&mut actual, k);
                assert_eq!(actual[k], expected[k]);
                assert!(actual[..k].iter().all(|x| x <= &actual[k]));
                assert!(actual[k + 1..].iter().all(|x| x >= &actual[k]));
                actual.sort_unstable();
                assert_eq!(actual, expected);
            }
            group.bench_function(BenchmarkId::new($name, inputs[0].len()), |b| {
                let mut next = 0;
                b.iter_batched_ref(
                    || {
                        let values = inputs[next % inputs.len()].clone();
                        next += 1;
                        values
                    },
                    |values| {
                        $select(black_box(values), black_box(k));
                    },
                    BatchSize::LargeInput,
                );
            });
        }};
    }
    register!("original", original);
    register!("kth", kth::partition_by_kth);
    register!("std", standard);
    group.finish();
}

fn selections(c: &mut Criterion) {
    for n in sizes::sizes() {
        let mut rng = StdRng::seed_from_u64(42);
        let random: Vec<Vec<u64>> = (0..16)
            .map(|_| (0..n).map(|_| rng.random()).collect())
            .collect();
        let random_bytes: Vec<Vec<u8>> = random
            .iter()
            .map(|v| v.iter().map(|x| *x as u8).collect())
            .collect();
        for (rank, k) in [
            ("min", 0),
            ("p01", n / 100),
            ("median", n / 2),
            ("max", n - 1),
        ] {
            bench(c, "u64/random", &random, rank, k);
            bench(c, "u8/random", &random_bytes, rank, k);
        }
        let words: Vec<Vec<String>> = random
            .iter()
            .map(|v| v.iter().map(|x| format!("{x:016x}")).collect())
            .collect();
        bench(c, "String/random", &words, "median", n / 2);
        let wide: Vec<Vec<[u64; 8]>> = random
            .iter()
            .map(|v| v.iter().map(|x| [*x; 8]).collect())
            .collect();
        bench(c, "64-byte/random", &wide, "median", n / 2);
        for (case, values) in [
            ("sorted", (0..n as u64).collect::<Vec<_>>()),
            ("reverse", (0..n as u64).rev().collect()),
            (
                "organ-pipe",
                (0..n).map(|i| i.min(n - 1 - i) as u64).collect(),
            ),
            ("sawtooth", (0..n).map(|i| (i % 7) as u64).collect()),
            ("equal", vec![0; n]),
            ("leading-one", {
                let mut v = vec![0; n];
                v[0] = 1;
                v
            }),
            ("trailing-one", {
                let mut v = vec![0; n];
                v[n - 1] = 1;
                v
            }),
        ] {
            let bytes: Vec<u8> = values.iter().map(|x| *x as u8).collect();
            bench(c, &format!("u64/{case}"), &[values], "median", n / 2);
            bench(c, &format!("u8/{case}"), &[bytes], "median", n / 2);
        }
    }
}

criterion_group!(selection, selections);
criterion_main!(selection);
