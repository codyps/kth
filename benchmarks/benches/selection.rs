//! Compare selection algorithms without operation-count overhead.
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;

#[cfg(feature = "experiments")]
#[path = "support/adaptive.rs"]
mod adaptive;

#[path = "support/cyclic_baseline.rs"]
mod cyclic_baseline;
#[cfg(feature = "experiments")]
#[path = "support/experiments.rs"]
mod experiments;
#[allow(dead_code)]
mod inputs;
#[path = "support/rank_baseline.rs"]
mod rank_baseline;
#[path = "support/selection_cases.rs"]
mod selection_cases;
mod sizes;
#[path = "support/swap_baseline.rs"]
mod swap_baseline;

fn original<T: Ord>(values: &mut [T], k: usize) {
    kth::quickselect::quickselect(kth::quickselect::repeated_step3, values, k);
}

fn standard<T: Ord>(values: &mut [T], k: usize) {
    values.select_nth_unstable(k);
}

struct Timing<'a>(&'a mut Criterion);

impl selection_cases::Runner for Timing<'_> {
    fn run<T: Ord + Clone + std::fmt::Debug>(
        &mut self,
        name: &str,
        inputs: &[Vec<T>],
        rank: &str,
        k: usize,
    ) {
        let mut group = self.0.benchmark_group(format!("selection/{name}/{rank}"));
        if std::env::var_os("KTH_BENCH_FLAT").is_some() {
            group.sampling_mode(criterion::SamplingMode::Flat);
        }
        group.throughput(Throughput::Elements(inputs[0].len() as u64));
        macro_rules! register {
            ($name:literal, $select:path) => {{
                let mut validated = false;
                group.bench_function(BenchmarkId::new($name, inputs[0].len()), |b| {
                    if !validated {
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
                        validated = true;
                    }
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
                        if std::mem::size_of_val(inputs[0].as_slice()) >= 1 << 20 {
                            BatchSize::PerIteration
                        } else {
                            BatchSize::LargeInput
                        },
                    );
                });
            }};
        }
        register!("original", original);
        if std::env::var_os("KTH_BENCH_SWAP_FIRST").is_some() {
            register!("swap", swap_baseline::select);
            register!("kth", kth::partition_by_kth);
        } else {
            register!("kth", kth::partition_by_kth);
            register!("swap", swap_baseline::select);
        }
        register!("cyclic", cyclic_baseline::select);
        register!("rank-before", rank_baseline::select);
        register!("std", standard);
        #[cfg(feature = "experiments")]
        {
            register!("control", experiments::control);
            register!("small16", experiments::small16);
            register!("small32", experiments::small32);
            register!("fr-window", experiments::fr_window);
            register!("expand", experiments::expand);
            register!("block64", experiments::block64);
            register!("block128", experiments::block128);
            register!("ancestor", experiments::ancestor);
            register!("ancestor-sample", experiments::ancestor_sample);
            register!("sesquick", experiments::sesquick);
            register!("fr-block", experiments::fr_block);
            register!("ninther-all", experiments::ninther_all);
            register!("fr-sample", experiments::fr_sample);
            register!("fr-sample-block", experiments::fr_sample_block);
            register!("network16", experiments::network16);
            register!("isolated", kth::benchmark_adaptive_partition_by_kth);
            register!("adaptive-control", adaptive::adaptive_control);
            register!("stratified-mid", adaptive::stratified_mid);
            register!("adaptive-block", adaptive::adaptive_block);
            register!("stratified-block", adaptive::stratified_block);
            register!("stratified-all", adaptive::stratified_all);
            register!("guarded", adaptive::guarded);
            register!("strict-duplicates", adaptive::strict_duplicates);
            register!("bounded-duplicates", adaptive::bounded_duplicates);
            register!("guarded-cyclic", adaptive::guarded_cyclic);
            register!("combined", adaptive::combined);
            register!("verified-duplicates", adaptive::verified_duplicates);
            register!("verified-guarded", adaptive::verified_guarded);
            register!("verified-control", adaptive::verified_control);
            register!("verified8", adaptive::verified8);
            register!("verified32", adaptive::verified32);
        }
        group.finish();
    }
}

fn selections(c: &mut Criterion) {
    if cfg!(feature = "instrumentation") {
        panic!(
            "operation counters distort timings; run selection without --features instrumentation"
        );
    }
    let config = selection_cases::Config::from_env();
    selection_cases::visit(&config, &mut Timing(c));
}

criterion_group!(selection, selections);
criterion_main!(selection);
