// Frozen selector from 4f9c8a6, before cyclic partitioning.
// Algorithm frozen; imports and diagnostic hooks may differ from that revision.
// Sampled quickselect with a deterministic linear-time fallback.

use core::{cmp::Ordering, mem::size_of};
use kth::instrumentation::{self, Phase};
use kth::quickselect::{hoare_partition, median3_index, quickselect, repeated_step3};

pub(crate) fn select<T: Ord>(a: &mut [T], k: usize) {
    assert!(
        k < a.len(),
        "the order index must be smaller than the slice length"
    );
    if size_of::<T>() == 0 {
        return;
    }
    select_with_budget(a, k, a.len().saturating_mul(4));
}

fn select_with_budget<T: Ord>(mut a: &mut [T], mut k: usize, mut budget: usize) {
    loop {
        let _partition_phase = instrumentation::phase(Phase::Partition);
        let n = a.len();
        if n == 1 {
            return;
        }
        // Extreme ranks need only a scan, including after narrowing the slice.
        if k == 0 {
            instrumentation::scanned(n - 1);
            let p = (1..n).fold(0, |p, i| if a[i] < a[p] { i } else { p });
            instrumentation::swap(a, 0, p);
            return;
        }
        if k == n - 1 {
            instrumentation::scanned(n - 1);
            let p = (1..n).fold(0, |p, i| if a[i] > a[p] { i } else { p });
            instrumentation::swap(a, n - 1, p);
            return;
        }
        // Keep the existing selector for small slices. Charge each sampled pass
        // for its whole active slice, rather than limiting recursion depth:
        // at most 4 * original_len partition elements are processed before
        // the O(n) fallback. Pivot sampling is also at most linear per pass.
        if n <= 64 || budget < n {
            instrumentation::dispatch(n <= 64);
            let _fallback_phase = instrumentation::phase(Phase::Fallback);
            quickselect(repeated_step3, a, k);
            return;
        }
        budget -= n;

        let sampling_phase = instrumentation::phase(Phase::Sampling);
        let step = n / 9;
        let (x, y, z) = if size_of::<T>() > 16 && size_of::<T>() <= 32 {
            // More accurate pivots reduce expensive comparisons and element
            // movement. The first/middle/last regions also include the exact
            // middle position on monotonic inputs.
            (
                sample_median(a, 0, step),
                sample_median(a, n / 2 - step / 2, step),
                sample_median(a, n - step, step),
            )
        } else {
            // Preserve the established pivots for scalar and large elements.
            // Recursive sampling can interact poorly with low-cardinality
            // integer inputs.
            (
                median3_index(a, 0, step, 2 * step),
                median3_index(a, 3 * step, 4 * step, 5 * step),
                median3_index(a, 6 * step, 7 * step, 8 * step),
            )
        };
        let pivot = median3_index(a, x, y, z);
        let repeated = a[x] == a[y] || a[y] == a[z] || a[x] == a[z];
        drop(sampling_phase);
        if repeated {
            // A repeated sample suggests many duplicates. Discard the whole
            // equal band instead of repeatedly selecting among equal elements.
            let (lo, hi) = partition_equal(a, pivot);
            if k < lo {
                a = &mut a[..lo];
            } else if k >= hi {
                a = &mut a[hi..];
                k -= hi;
            } else {
                return;
            }
            continue;
        }

        // Unconditional swaps avoid an unpredictable branch for moderately sized
        // elements, including String. For larger elements, Hoare moves less data.
        let p = if size_of::<T>() <= 32 {
            partition(a, pivot)
        } else {
            hoare_partition(a, pivot)
        };
        if p == k {
            return;
        }
        if p > k {
            a = &mut a[..p];
        } else {
            a = &mut a[p + 1..];
            k -= p + 1;
        }
    }
}

fn partition<T: Ord>(a: &mut [T], pivot: usize) -> usize {
    instrumentation::partition_pass();
    instrumentation::scanned(a.len() - 1);
    instrumentation::swap(a, 0, pivot);
    let (pivot, tail) = a.split_first_mut().unwrap();
    let mut count = 0;
    if size_of::<T>() <= 8 {
        let mut i = 0;
        while i + 1 < tail.len() {
            let first = tail[i] < *pivot;
            let second = tail[i + 1] < *pivot;
            // count <= i, so the first swap cannot change tail[i + 1].
            // Classifying the pair before moving it shortens the dependency chain.
            instrumentation::swap(tail, i, count);
            count += usize::from(first);
            instrumentation::swap(tail, i + 1, count);
            count += usize::from(second);
            i += 2;
        }
        if i < tail.len() {
            let less = tail[i] < *pivot;
            instrumentation::swap(tail, i, count);
            count += usize::from(less);
        }
    } else {
        for i in 0..tail.len() {
            let less = tail[i] < *pivot;
            // count <= i. When less is false, both exchanged values are >= pivot;
            // otherwise this extends the strictly smaller prefix by one element.
            instrumentation::swap(tail, i, count);
            count += usize::from(less);
        }
    }
    instrumentation::swap(a, 0, count);
    count
}

// Return the half-open range containing all values equal to the pivot.
fn partition_equal<T: Ord>(a: &mut [T], pivot: usize) -> (usize, usize) {
    instrumentation::partition_pass();
    instrumentation::scanned(a.len() - 1);
    instrumentation::swap(a, 0, pivot);
    let (pivot, tail) = a.split_first_mut().unwrap();
    let (mut lo, mut i, mut hi) = (0, 0, tail.len());
    while i < hi {
        match tail[i].cmp(pivot) {
            Ordering::Less => {
                instrumentation::swap(tail, lo, i);
                lo += 1;
                i += 1;
            }
            Ordering::Equal => i += 1,
            Ordering::Greater => {
                hi -= 1;
                instrumentation::swap(tail, i, hi);
            }
        }
    }
    instrumentation::swap(a, 0, lo);
    (lo, hi + 1)
}

// Select a pseudomedian from three disjoint regions. Each recursion samples
// three regions of at most len / 9, so it inspects O(sqrt(len)) elements and
// stays within the linear work charged for a partition pass. len is nonzero,
// and start..start + len lies inside a at every recursion level.
fn sample_median<T: Ord>(a: &[T], start: usize, len: usize) -> usize {
    if len < 64 {
        return median3_index(a, start, start + len / 2, start + len - 1);
    }
    let step = len / 9;
    let x = sample_median(a, start, step);
    let y = sample_median(a, start + len / 2 - step / 2, step);
    let z = sample_median(a, start + len - step, step);
    median3_index(a, x, y, z)
}
