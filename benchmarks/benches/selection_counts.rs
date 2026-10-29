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
        for algorithm in ["original", "swap", "cyclic", "rank-before", "kth", "std"] {
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
                    let _phase = instrumentation::phase(if algorithm == "original" {
                        Phase::Fallback
                    } else {
                        Phase::Other
                    });
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
                    budget_fallbacks,
                    opaque_sort_calls
                );
                fallback_calls += usize::from(counts.budget_fallbacks > 0);
            }
            let calls = inputs.len();
            let elements = calls as f64 * n as f64;
            // std's internals are not instrumented; empty cells mean unavailable, never zero.
            let opaque = sum.opaque_sort_calls > 0;
            let internal = if algorithm == "std" {
                vec![String::new(); 8].join(",")
            } else {
                format!(
                    "{},{},{},{},{},{},{},{}",
                    sum.sampling_comparisons,
                    sum.partition_comparisons,
                    sum.fallback_comparisons,
                    if opaque {
                        String::new()
                    } else {
                        sum.elements_scanned.to_string()
                    },
                    if opaque {
                        String::new()
                    } else {
                        sum.bytes_moved.to_string()
                    },
                    sum.partition_passes,
                    sum.small_slice_dispatches,
                    sum.budget_fallbacks
                )
            };
            let derived = if algorithm == "std" {
                ",,".into()
            } else if opaque {
                format!(",,{:.6}", fallback_calls as f64 / calls as f64)
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
                "{id},{k},{},{},{calls},{},{internal},{:.6},{derived},{}",
                size_of::<T>(),
                n * size_of::<T>(),
                sum.comparisons,
                sum.comparisons as f64 / elements,
                sum.opaque_sort_calls
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
    writeln!(counter.output, "benchmark,k,element_bytes,array_bytes,calls,comparisons,sampling_comparisons,partition_comparisons,fallback_comparisons,elements_scanned,bytes_moved,partition_passes,small_slice_dispatches,budget_fallbacks,comparisons_per_element,scans_per_element,bytes_moved_per_element,budget_fallback_rate,opaque_sort_calls").unwrap();
    selection_cases::visit(&config, &mut counter);
    assert!(
        counter.rows > 0,
        "KTH_BENCH_FILTER did not match any workloads"
    );
    counter.output.flush().unwrap();
    eprintln!("{} diagnostic rows; profile={}, inputs={}; counts are totals over calls, rates are per call", counter.rows, config.profile, config.input_count);
}
