//! Integer-only selectors, timed with the same cloned-input protocol.
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use std::{fmt::Debug, hint::black_box};
#[path = "support/byte_tuning.rs"]
mod byte_tuning;
#[allow(dead_code)]
mod inputs;
#[path = "support/numeric_experiments.rs"]
mod numeric_experiments;
mod sizes;

fn bench<T: Ord + Clone + Debug>(
    c: &mut Criterion,
    name: &str,
    bank: &[Vec<T>],
    rank: &str,
    k: usize,
    algorithm: &str,
    select: fn(&mut [T], usize),
) {
    let mut group = c.benchmark_group(format!("selection/{name}/{rank}"));
    if std::env::var_os("KTH_BENCH_FLAT").is_some() {
        group.sampling_mode(criterion::SamplingMode::Flat);
    }
    group.throughput(Throughput::Elements(bank[0].len() as u64));
    let mut validated = false;
    group.bench_function(BenchmarkId::new(algorithm, bank[0].len()), |b| {
        if !validated {
            for input in bank {
                let mut expected = input.clone();
                expected.sort_unstable();
                let mut actual = input.clone();
                select(&mut actual, k);
                assert_eq!(actual[k], expected[k]);
                assert!(actual[..k].iter().all(|v| v <= &actual[k]));
                assert!(actual[k + 1..].iter().all(|v| v >= &actual[k]));
                actual.sort_unstable();
                assert_eq!(actual, expected);
            }
            validated = true;
        }
        let mut next = 0;
        b.iter_batched_ref(
            || {
                let a = bank[next % bank.len()].clone();
                next += 1;
                a
            },
            |a| select(black_box(a), black_box(k)),
            if std::mem::size_of_val(bank[0].as_slice()) >= 1 << 20 {
                BatchSize::PerIteration
            } else {
                BatchSize::LargeInput
            },
        );
    });
    group.finish();
}

fn standard<T: Ord>(a: &mut [T], k: usize) {
    a.select_nth_unstable(k);
}

fn suite(c: &mut Criterion) {
    if cfg!(feature = "instrumentation") {
        panic!("timing requires instrumentation disabled");
    }
    let count = std::env::var("KTH_BENCH_INPUTS").map_or(16, |v| v.parse().unwrap());
    let ranks = std::env::var("KTH_BENCH_RANKS").unwrap_or_else(|_| "p01,median,p99".into());
    let types = std::env::var("KTH_BENCH_TYPES").unwrap_or_else(|_| "u8,u64".into());
    for n in sizes::sizes() {
        let random = inputs::random_bank_of(n, count, |v| v);
        for case in [
            "random",
            "sawtooth",
            "equal",
            "sorted",
            "reverse",
            "dominant",
            "clustered",
        ] {
            let structured = vec![(0..n)
                .map(|i| match case {
                    "sawtooth" => (i % 7) as u64,
                    "equal" => 0,
                    "sorted" => i as u64,
                    "reverse" => (n - i) as u64,
                    "dominant" if i % 10 != 0 => 5,
                    "clustered" if i < n * 9 / 10 => 5,
                    _ => (i as u64).wrapping_mul(6364136223846793005),
                })
                .collect::<Vec<_>>()];
            let bank = if case == "random" {
                &random
            } else {
                &structured
            };
            let bytes: Vec<Vec<u8>> = bank
                .iter()
                .map(|a| a.iter().map(|v| *v as u8).collect())
                .collect();
            for rank in ranks.split(',') {
                let k = match rank {
                    "min" => 0,
                    "next" => 1.min(n - 1),
                    "median" => n / 2,
                    "max" => n - 1,
                    "p01" => n / 100,
                    "p10" => n / 10,
                    "p90" => n / 10 * 9 + n % 10 * 9 / 10,
                    "p99" => n / 100 * 99 + n % 100 * 99 / 100,
                    _ => panic!("unsupported numeric rank: {rank}"),
                };
                if types.split(',').any(|v| v == "u8") {
                    for (algorithm, select) in [
                        ("kth", kth::partition_by_kth as fn(&mut [u8], usize)),
                        ("kth-u8", kth::partition_u8_by_kth),
                        ("byte-before", byte_tuning::baseline),
                        ("compact16", byte_tuning::compact),
                        ("lanes8-16", byte_tuning::lanes8),
                        ("merged32", byte_tuning::merged),
                        ("bitmap32", byte_tuning::bitmap),
                        ("entropy8", byte_tuning::entropy8),
                        ("std", standard),
                        ("histogram", numeric_experiments::histogram),
                        ("histogram4", numeric_experiments::histogram4),
                        ("counting", numeric_experiments::counting),
                        ("counting4", numeric_experiments::counting4),
                    ] {
                        bench(c, &format!("u8/{case}"), &bytes, rank, k, algorithm, select);
                    }
                }
                if types.split(',').any(|v| v == "u64") {
                    for (algorithm, select) in [
                        ("kth", kth::partition_by_kth as fn(&mut [u64], usize)),
                        ("std", standard),
                        ("radix8", numeric_experiments::radix8),
                        ("radix11", numeric_experiments::radix11),
                    ] {
                        bench(c, &format!("u64/{case}"), bank, rank, k, algorithm, select);
                    }
                    #[cfg(feature = "native-simd")]
                    if numeric_experiments::simd_available() {
                        bench(
                            c,
                            &format!("u64/{case}"),
                            bank,
                            rank,
                            k,
                            "x86-simd",
                            numeric_experiments::simd,
                        );
                    }
                }
            }
        }
    }
}
criterion_group!(benches, suite);
criterion_main!(benches);
