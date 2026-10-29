#![cfg(feature = "instrumentation")]

#[path = "../benches/support/cyclic_baseline.rs"]
mod cyclic_baseline;
#[path = "../benches/support/swap_baseline.rs"]
mod swap_baseline;

#[test]
fn cyclic_and_swap_counts_match_single_partition_transfer_costs() {
    // The sorted ninther selects index 28 at n=65. Selecting that exact rank
    // returns after one partition, isolating both kernels from later pivots.
    for (select, transfers) in [
        (cyclic_baseline::select as fn(&mut [u64], usize), 2 * 65 + 5),
        (swap_baseline::select as fn(&mut [u64], usize), 3 * (65 + 1)),
        (kth::partition_by_kth as fn(&mut [u64], usize), 3 * (65 + 1)),
    ] {
        let mut values: Vec<u64> = (0..65).collect();
        kth::instrumentation::reset();
        select(&mut values, 28);
        let counts = kth::instrumentation::snapshot();
        assert_eq!(values[28], 28);
        assert_eq!(counts.partition_passes, 1);
        assert_eq!(counts.elements_scanned, 64);
        assert_eq!(counts.bytes_moved, transfers * 8);
        assert_eq!(counts.small_slice_dispatches, 0);
        assert_eq!(counts.budget_fallbacks, 0);
    }
}
