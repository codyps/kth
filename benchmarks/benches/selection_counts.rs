//! Untimed operation counts on the same workload bank as the selection benchmark.
use kth::instrumentation::{self, Counts, Phase};
use std::{
    cmp::Ordering,
    fmt::Debug,
    io::{self, Write},
    mem::size_of,
};

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

// No extra fields: preserve size-dependent selector dispatch, including String.
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
struct Counted<T>(T);

impl<T: Ord> Ord for Counted<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        instrumentation::comparison();
        self.0.cmp(&other.0)
    }
}

impl<T: Ord> PartialOrd for Counted<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord> PartialEq for Counted<T> {
    fn eq(&self, other: &Self) -> bool {
        instrumentation::comparison();
        self.0.eq(&other.0)
    }
}

impl<T: Ord> Eq for Counted<T> {}

struct Counter<W> {
    output: W,
    filter: String,
    ids: Option<std::collections::BTreeSet<String>>,
    rows: usize,
}

impl<W: Write> selection_cases::Runner for Counter<W> {
    fn run<T: Ord + Clone + Debug>(&mut self, name: &str, inputs: &[Vec<T>], rank: &str, k: usize) {
        let algorithms = ["original", "swap", "cyclic", "rank-before", "kth", "std"];
        #[cfg(feature = "experiments")]
        let algorithms = [
            algorithms.as_slice(),
            &[
                "control",
                "small16",
                "small32",
                "fr-window",
                "expand",
                "block64",
                "block128",
                "ancestor",
                "ancestor-sample",
                "sesquick",
                "fr-block",
                "ninther-all",
                "fr-sample",
                "fr-sample-block",
                "network16",
            ],
        ]
        .concat();
        for algorithm in algorithms {
            let n = inputs[0].len();
            let id = format!("selection/{name}/{rank}/{algorithm}/{n}");
            if !id.contains(&self.filter) || self.ids.as_ref().is_some_and(|ids| !ids.contains(&id))
            {
                continue;
            }
            let mut sum = Counts::default();
            let mut fallback_calls = 0;
            for input in inputs {
                let mut expected = input.clone();
                expected.sort_unstable();
                let mut values: Vec<_> = input.iter().cloned().map(Counted).collect();
                instrumentation::reset();
                {
                    let _phase = instrumentation::phase(
                        if algorithm == "original" || algorithm == "ninther-all" {
                            Phase::Fallback
                        } else {
                            Phase::Other
                        },
                    );
                    match algorithm {
                        "original" => kth::quickselect::quickselect(
                            kth::quickselect::repeated_step3,
                            &mut values,
                            k,
                        ),
                        "swap" => swap_baseline::select(&mut values, k),
                        "cyclic" => cyclic_baseline::select(&mut values, k),
                        "rank-before" => rank_baseline::select(&mut values, k),
                        "kth" => kth::partition_by_kth(&mut values, k),
                        "std" => {
                            values.select_nth_unstable(k);
                        }
                        #[cfg(feature = "experiments")]
                        "control" => experiments::control(&mut values, k),
                        #[cfg(feature = "experiments")]
                        "small16" => experiments::small16(&mut values, k),
                        #[cfg(feature = "experiments")]
                        "small32" => experiments::small32(&mut values, k),
                        #[cfg(feature = "experiments")]
                        "fr-window" => experiments::fr_window(&mut values, k),
                        #[cfg(feature = "experiments")]
                        "expand" => experiments::expand(&mut values, k),
                        #[cfg(feature = "experiments")]
                        "block64" => experiments::block64(&mut values, k),
                        #[cfg(feature = "experiments")]
                        "block128" => experiments::block128(&mut values, k),
                        #[cfg(feature = "experiments")]
                        "ancestor" => experiments::ancestor(&mut values, k),
                        #[cfg(feature = "experiments")]
                        "ancestor-sample" => experiments::ancestor_sample(&mut values, k),
                        #[cfg(feature = "experiments")]
                        "sesquick" => experiments::sesquick(&mut values, k),
                        #[cfg(feature = "experiments")]
                        "fr-block" => experiments::fr_block(&mut values, k),
                        #[cfg(feature = "experiments")]
                        "ninther-all" => experiments::ninther_all(&mut values, k),
                        #[cfg(feature = "experiments")]
                        "fr-sample" => experiments::fr_sample(&mut values, k),
                        #[cfg(feature = "experiments")]
                        "fr-sample-block" => experiments::fr_sample_block(&mut values, k),
                        #[cfg(feature = "experiments")]
                        "network16" => experiments::network16(&mut values, k),
                        _ => unreachable!(),
                    }
                }
                // Take the snapshot before validation comparisons or sorting.
                let counts = instrumentation::snapshot();
                assert_eq!(values[k].0, expected[k], "{id}");
                assert!(values[..k].iter().all(|v| v.0 <= values[k].0), "{id}");
                assert!(values[k + 1..].iter().all(|v| v.0 >= values[k].0), "{id}");
                values.sort_unstable();
                assert!(values.iter().zip(&expected).all(|(v, e)| &v.0 == e), "{id}");
                macro_rules! add { ($($field:ident),+) => { $(sum.$field += counts.$field;)+ }; }
                add!(
                    comparisons,
                    sampling_comparisons,
                    partition_comparisons,
                    fallback_comparisons,
                    elements_scanned,
                    bytes_moved,
                    partition_passes,
                    small_slice_dispatches,
                    budget_fallbacks
                );
                fallback_calls += usize::from(counts.budget_fallbacks > 0);
            }
            let calls = inputs.len();
            let elements = calls as f64 * n as f64;
            // std's internals are not instrumented; empty cells mean unavailable, never zero.
            let internal = if algorithm == "std" {
                vec![String::new(); 8].join(",")
            } else {
                format!(
                    "{},{},{},{},{},{},{},{}",
                    sum.sampling_comparisons,
                    sum.partition_comparisons,
                    sum.fallback_comparisons,
                    sum.elements_scanned,
                    sum.bytes_moved,
                    sum.partition_passes,
                    sum.small_slice_dispatches,
                    sum.budget_fallbacks
                )
            };
            let derived = if algorithm == "std" {
                ",,".into()
            } else {
                format!(
                    "{:.6},{:.6},{:.6}",
                    sum.elements_scanned as f64 / elements,
                    sum.bytes_moved as f64 / elements,
                    fallback_calls as f64 / calls as f64
                )
            };
            writeln!(
                self.output,
                "{id},{k},{},{},{calls},{},{internal},{:.6},{derived}",
                size_of::<T>(),
                n * size_of::<T>(),
                sum.comparisons,
                sum.comparisons as f64 / elements
            )
            .unwrap();
            self.rows += 1;
        }
    }
}

fn main() {
    let config = selection_cases::Config::from_env();
    let filter = std::env::var("KTH_BENCH_FILTER").unwrap_or_default();
    let ids = std::env::var_os("KTH_BENCH_IDS").map(|path| {
        std::fs::read_to_string(path)
            .expect("read KTH_BENCH_IDS")
            .lines()
            .map(str::to_owned)
            .collect()
    });
    let mut counter = Counter {
        output: io::BufWriter::new(io::stdout().lock()),
        filter,
        ids,
        rows: 0,
    };
    writeln!(counter.output, "benchmark,k,element_bytes,array_bytes,calls,comparisons,sampling_comparisons,partition_comparisons,fallback_comparisons,elements_scanned,bytes_moved,partition_passes,small_slice_dispatches,budget_fallbacks,comparisons_per_element,scans_per_element,bytes_moved_per_element,budget_fallback_rate").unwrap();
    selection_cases::visit(&config, &mut counter);
    assert!(
        counter.rows > 0,
        "KTH_BENCH_FILTER did not match any workloads"
    );
    counter.output.flush().unwrap();
    eprintln!("{} diagnostic rows; profile={}, inputs={}; counts are totals over calls, rates are per call", counter.rows, config.profile, config.input_count);
}
