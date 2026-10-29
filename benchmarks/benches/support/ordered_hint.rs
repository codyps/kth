//! Benchmark-only ascending pivot hint, based on the fbb3235 selector.
//! Sampled quickselect with a deterministic linear-time fallback.

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
    // Optimize small inputs independently of the existing large-input tails.
    // Keep the original single-scan paths for the minimum and maximum.
    if a.len() <= 64 && k != 0 && k != a.len() - 1 {
        instrumentation::dispatch(true);
        select_small(a, k);
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
        let (pivot, repeated) = choose_pivot(a, k);
        drop(sampling_phase);
        if repeated {
            // A repeated sample suggests many duplicates. Discard the whole
            // equal band instead of repeatedly selecting among equal elements.
            // For small elements, a low-rank pivot often leaves a mostly-greater
            // suffix. Skip greater/greater swaps there. Keeping the original loop
            // for larger types also keeps their partition driver small.
            let (lo, hi) = if size_of::<T>() <= 8 && k <= n / 8 {
                partition_equal::<_, true>(a, pivot)
            } else {
                partition_equal::<_, false>(a, pivot)
            };
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

#[inline(always)]
fn select_small<T: Ord>(mut a: &mut [T], mut k: usize) {
    let _partition_phase = instrumentation::phase(Phase::Partition);
    // Even adversarial pivot choices are bounded by this fixed slice length.
    debug_assert!(a.len() <= 64);
    let n = a.len();
    // The original pivot remains competitive for this short scalar range.
    if size_of::<T>() <= 8 && n > 8 && n <= 16 {
        let _fallback_phase = instrumentation::phase(Phase::Fallback);
        quickselect(repeated_step3, a, k);
        return;
    }
    if n > 8 {
        let sampling_phase = instrumentation::phase(Phase::Sampling);
        let first_mid_equal = a[0] == a[n / 2];
        let mid_last_equal = a[n / 2] == a[n - 1];
        // Two matching samples with a distinct third can bias median-of-three
        // toward an extreme value. Keep the existing pivot on these inputs.
        let use_original =
            first_mid_equal != mid_last_equal || (!first_mid_equal && a[0] == a[n - 1]);
        drop(sampling_phase);
        if use_original {
            let _fallback_phase = instrumentation::phase(Phase::Fallback);
            quickselect(repeated_step3, a, k);
            return;
        }
    }
    loop {
        let n = a.len();
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
        // Sort tiny slices of small elements, but avoid fully sorting larger
        // elements. Below nine elements the repeated-step selector already
        // uses a simple midpoint pivot.
        if n <= 8 {
            if size_of::<T>() <= 8 {
                instrumentation::opaque_sort();
                a.sort_unstable();
            } else {
                let _fallback_phase = instrumentation::phase(Phase::Fallback);
                quickselect(repeated_step3, a, k);
            }
            return;
        }
        let mid = n / 2;
        // Bias the three-element sample toward the requested rank after
        // narrowing, instead of always choosing its median.
        let sampling_phase = instrumentation::phase(Phase::Sampling);
        let pivot = if k < n / 4 {
            let p = if a[0] < a[mid] { 0 } else { mid };
            if a[p] < a[n - 1] {
                p
            } else {
                n - 1
            }
        } else if k >= n - n / 4 {
            let p = if a[0] > a[mid] { 0 } else { mid };
            if a[p] > a[n - 1] {
                p
            } else {
                n - 1
            }
        } else {
            median3_index(a, 0, mid, n - 1)
        };
        drop(sampling_phase);
        let p = hoare_partition(a, pivot);
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

// Keep sampling separate so the larger rank policy does not inflate the hot
// partition driver. One call per pass is small compared with scanning the slice.
#[inline(never)]
fn choose_pivot<T: Ord>(a: &[T], k: usize) -> (usize, bool) {
    let n = a.len();
    let step = n / 9;
    let from_right = n - 1 - k;
    let (x, y, z) = if k <= n / 8 {
        // Tail ranks benefit from pivots biased toward the requested end.
        // Sample the centers of nine regions, avoiding endpoint pivots on
        // monotonic input. A median of minima/maxima is less extreme than
        // taking the minimum/maximum of the whole sample.
        sample_extrema::<T, true>(a, step)
    } else if from_right <= n / 8 {
        sample_extrema::<T, false>(a, step)
    } else if size_of::<T>() > 16 && size_of::<T>() <= 32 {
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
    // At the outermost ranks the sample's minimum/maximum usually removes
    // more work. Otherwise take the median of the three representatives.
    // These are heuristics; the unchanged work budget protects the O(n) bound.
    let pivot = if k <= n / 84 {
        min3_index(a, x, y, z)
    } else if from_right <= n / 84 {
        max3_index(a, x, y, z)
    } else {
        median3_index(a, x, y, z)
    };
    let repeated = a[x] == a[y] || a[y] == a[z] || a[x] == a[z];
    if size_of::<T>() == 8
        && n >= 16384
        && !repeated
        && k > n / 8
        && from_right > n / 8
        && a[0] < a[n / 2]
        && a[n / 2] < a[n - 1]
        && ordered_hint(a)
    {
        return (k, false);
    }
    (pivot, repeated)
}

fn min3_index<T: Ord>(a: &[T], x: usize, y: usize, z: usize) -> usize {
    let p = if a[x] < a[y] { x } else { y };
    if a[p] < a[z] {
        p
    } else {
        z
    }
}

fn max3_index<T: Ord>(a: &[T], x: usize, y: usize, z: usize) -> usize {
    let p = if a[x] > a[y] { x } else { y };
    if a[p] > a[z] {
        p
    } else {
        z
    }
}

fn sample_extrema<T: Ord, const MIN: bool>(a: &[T], step: usize) -> (usize, usize, usize) {
    let start = step / 2;
    let extreme = |x, y, z| {
        if MIN {
            min3_index(a, x, y, z)
        } else {
            max3_index(a, x, y, z)
        }
    };
    (
        extreme(start, start + step, start + 2 * step),
        extreme(start + 3 * step, start + 4 * step, start + 5 * step),
        extreme(start + 6 * step, start + 7 * step, start + 8 * step),
    )
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
fn partition_equal<T: Ord, const SKIP_GREATER: bool>(a: &mut [T], pivot: usize) -> (usize, usize) {
    instrumentation::partition_pass();
    instrumentation::scanned(a.len() - 1);
    instrumentation::swap(a, 0, pivot);
    let (pivot, tail) = a.split_first_mut().unwrap();
    let (mut lo, mut i, mut hi) = (0, 0, tail.len());
    'partition: while i < hi {
        let mut order = tail[i].cmp(pivot);
        if SKIP_GREATER && order == Ordering::Greater {
            // The right side often already contains greater values, especially
            // with a pivot near the minimum. Skip them instead of swapping two
            // greater values and then classifying the same slot again.
            loop {
                hi -= 1;
                if i == hi {
                    break 'partition;
                }
                order = tail[hi].cmp(pivot);
                if order != Ordering::Greater {
                    instrumentation::swap(tail, i, hi);
                    break;
                }
            }
        }
        match order {
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

// Jitter within disjoint strata avoids fixed-stride aliases on periodic data.
// This only chooses a pivot: the unchanged work budget protects against a
// misleading sample. Multiply-high reduction avoids integer division per probe.
#[inline(never)]
fn ordered_hint<T: Ord>(a: &[T]) -> bool {
    let width = a.len() / 16;
    let mut state = (a.len() as u64) ^ 0xa4093822299f31d0;
    let mut previous = 0;
    for stratum in 0..16 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let start = stratum * width;
        let length = if stratum == 15 {
            a.len() - start
        } else {
            width
        };
        let index = start + ((((state >> 32) as u128) * (length as u128)) >> 32) as usize;
        if stratum != 0 && a[previous] >= a[index] {
            return false;
        }
        previous = index;
    }
    true
}
