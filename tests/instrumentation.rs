#![cfg(feature = "bench-instrumentation")]

use kth::instrumentation::{self, Phase};
use std::cmp::Ordering;

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
struct Counted(u64);

impl Ord for Counted {
    fn cmp(&self, other: &Self) -> Ordering {
        instrumentation::comparison();
        self.0.cmp(&other.0)
    }
}
impl PartialOrd for Counted {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for Counted {}

#[test]
fn extreme_rank_counts_one_scan_and_one_swap() {
    instrumentation::reset();
    let mut values = [Counted(4), Counted(3), Counted(2), Counted(1)];
    kth::partition_by_kth(&mut values, 0);
    let counts = instrumentation::snapshot();
    assert_eq!(values[0].0, 1);
    assert_eq!(counts.comparisons, 3);
    assert_eq!(counts.partition_comparisons, 3);
    assert_eq!(counts.elements_scanned, 3);
    assert_eq!(counts.bytes_moved, 3 * 8);
    assert_eq!(counts.partition_passes, 0);
    assert_eq!(counts.small_slice_dispatches, 0);
    assert_eq!(counts.budget_fallbacks, 0);
}

#[test]
fn small_slice_dispatch_is_distinct_from_budget_exhaustion() {
    instrumentation::reset();
    let mut values = [Counted(3), Counted(2), Counted(1)];
    kth::partition_by_kth(&mut values, 1);
    let counts = instrumentation::snapshot();
    assert_eq!(values[1].0, 2);
    assert!(counts.comparisons > 0);
    assert_eq!(counts.partition_comparisons, counts.comparisons);
    assert_eq!(counts.fallback_comparisons, 0);
    assert_eq!(counts.opaque_sort_calls, 1);
    // These partial totals exclude the opaque library sort. The CSV exporter
    // marks scan/movement fields unavailable when opaque_sort_calls is nonzero.
    assert_eq!(counts.elements_scanned, 0);
    assert_eq!(counts.bytes_moved, 0);
    assert_eq!(counts.partition_passes, 0);
    assert_eq!(counts.small_slice_dispatches, 1);
    assert_eq!(counts.budget_fallbacks, 0);
}

#[test]
fn small_selector_assigns_all_comparisons_to_phases() {
    for n in [16, 24, 64] {
        instrumentation::reset();
        let mut values: Vec<_> = (0..n as u64).map(Counted).collect();
        kth::partition_by_kth(&mut values, n / 2);
        let counts = instrumentation::snapshot();
        assert_eq!(
            counts.comparisons,
            counts.sampling_comparisons
                + counts.partition_comparisons
                + counts.fallback_comparisons
        );
        assert_eq!(counts.small_slice_dispatches, 1);
        assert_eq!(counts.budget_fallbacks, 0);
        assert!(counts.elements_scanned > 0);
        assert!(counts.bytes_moved > 0);
        assert_eq!(counts.opaque_sort_calls, 0);
    }
}

#[test]
fn equal_band_counts_sampling_and_exactly_one_partition() {
    instrumentation::reset();
    let mut values = [Counted(7); 128];
    kth::partition_by_kth(&mut values, 64);
    let counts = instrumentation::snapshot();
    assert_eq!(counts.sampling_comparisons, 13);
    assert_eq!(counts.partition_comparisons, 127);
    assert_eq!(counts.comparisons, 140);
    assert_eq!(counts.elements_scanned, 127);
    assert_eq!(counts.bytes_moved, 6 * 8); // Both pivot swaps count, including the self-swap.
    assert_eq!(counts.partition_passes, 1);
    assert_eq!(counts.small_slice_dispatches, 0);
    assert_eq!(counts.budget_fallbacks, 0);
}

#[test]
fn nested_phases_restore_on_unwind_and_counters_are_thread_local() {
    instrumentation::reset();
    let outer = instrumentation::phase(Phase::Sampling);
    let _ = std::panic::catch_unwind(|| {
        let _inner = instrumentation::phase(Phase::Fallback);
        instrumentation::comparison();
        panic!("exercise phase restoration");
    });
    instrumentation::comparison();
    std::thread::spawn(|| {
        instrumentation::reset();
        assert_eq!(instrumentation::snapshot().comparisons, 0);
        instrumentation::comparison();
    })
    .join()
    .unwrap();
    let counts = instrumentation::snapshot();
    assert_eq!(counts.comparisons, 2);
    assert_eq!(counts.sampling_comparisons, 1);
    assert_eq!(counts.fallback_comparisons, 1);
    drop(outer);
    instrumentation::reset();
    assert_eq!(instrumentation::snapshot().comparisons, 0);
}
