//! Focused record experiments with the shared input and timing protocol.
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use std::{fmt::Debug, hint::black_box};
#[path = "support/before_followup.rs"]
mod before;
#[path = "support/block_tuning.rs"]
mod block;
#[allow(dead_code)]
mod inputs;
#[allow(dead_code)]
#[path = "support/selection_cases.rs"]
mod selection_cases;
mod sizes;

fn standard<T: Ord>(a: &mut [T], k: usize) {
    a.select_nth_unstable(k);
}

struct Timing<'a>(&'a mut Criterion);
impl selection_cases::Runner for Timing<'_> {
    fn run<T: Ord + Clone + Debug>(&mut self, name: &str, inputs: &[Vec<T>], rank: &str, k: usize) {
        let mut group = self.0.benchmark_group(format!("selection/{name}/{rank}"));
        if std::env::var_os("KTH_BENCH_FLAT").is_some() {
            group.sampling_mode(criterion::SamplingMode::Flat);
        }
        let n = inputs[0].len();
        group.throughput(Throughput::Elements(n as u64));
        for (algorithm, select) in [
            ("kth", kth::partition_by_kth as fn(&mut [T], usize)),
            ("std", standard),
            ("before-followup", before::select),
            ("fixed-entry", kth::benchmark_fixed_partition_by_kth),
            ("block-offsets", block::select),
            ("index-sample-block", block::indexed),
        ] {
            let mut validated = false;
            let mut next = 0;
            group.bench_function(BenchmarkId::new(algorithm, n), |b| {
                if !validated {
                    for input in inputs {
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
                b.iter_batched_ref(
                    || {
                        let a = inputs[next % inputs.len()].clone();
                        next += 1;
                        a
                    },
                    |a| select(black_box(a), black_box(k)),
                    if std::mem::size_of_val(inputs[0].as_slice()) >= 1 << 20 {
                        BatchSize::PerIteration
                    } else {
                        BatchSize::LargeInput
                    },
                );
            });
        }
        group.finish();
    }
}
fn stress<T: Ord + Clone + Debug>(
    c: &mut Criterion,
    name: &str,
    periods: &[u64],
    convert: impl Fn(u64) -> T,
) {
    let count: usize = std::env::var("KTH_BENCH_INPUTS").map_or(16, |v| v.parse().unwrap());
    assert!(count > 0);
    for n in sizes::sizes() {
        for &period in periods {
            let periodic = vec![(0..n).map(|i| convert((i as u64) % period)).collect()];
            let random = inputs::random_bank_of(n, count, |v| convert(v % period));
            selection_cases::Runner::run(
                &mut Timing(c),
                &format!("{name}/periodic{period}"),
                &periodic,
                "median",
                n / 2,
            );
            selection_cases::Runner::run(
                &mut Timing(c),
                &format!("{name}/random{period}"),
                &random,
                "median",
                n / 2,
            );
        }
    }
}

fn suite(c: &mut Criterion) {
    if cfg!(feature = "instrumentation") {
        panic!("timing requires instrumentation disabled");
    }
    if let Ok(value) = std::env::var("KTH_RECORD_PERIODS") {
        let periods: Vec<u64> = value.split(',').map(|v| v.parse().unwrap()).collect();
        assert!(!periods.is_empty() && periods.iter().all(|p| *p > 0));
        let types = std::env::var("KTH_BENCH_TYPES").unwrap_or_else(|_| "33-byte,64-byte".into());
        if types.split(',').any(|v| v == "128-byte") {
            stress(c, "128-byte", &periods, |v| [v; 16]);
        }
        if types.split(',').any(|v| v == "64-byte") {
            stress(c, "64-byte", &periods, |v| [v; 8]);
        }
        if types.split(',').any(|v| v == "33-byte") {
            stress(c, "33-byte", &periods, |v| -> [u8; 33] {
                std::array::from_fn(|i| v.to_be_bytes()[i % 8])
            });
        }
    } else {
        selection_cases::visit_records(&selection_cases::Config::from_env(), &mut Timing(c));
    }
}
criterion_group!(benches, suite);
criterion_main!(benches);
