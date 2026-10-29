//! Sampled quickselect with a deterministic linear-time fallback.

use crate::instrumentation::{self, Phase};
use crate::quickselect::{hoare_partition, median3_index, quickselect, repeated_step3};
use core::{cmp::Ordering, mem::size_of};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampled_medians_stay_inside_their_regions() {
        for n in (1..=512).chain([575, 576, 577, 5183, 5184, 5185, 46655, 46656, 46657, 65536]) {
            for start in [0, 1, 17] {
                let mut values: Vec<_> = (0..start + n + 23).collect();
                assert_eq!(sample_median(&values, start, n), start + n / 2);
                values.reverse();
                assert_eq!(sample_median(&values, start, n), start + n / 2);
            }
        }
    }

    #[test]
    fn exhausted_budget_uses_deterministic_selection() {
        for n in [65, 81, 256, 1024] {
            let input: Vec<_> = (0..n).map(|i| (i * 37) % 101).collect();
            let mut sorted = input.clone();
            sorted.sort_unstable();
            for k in [1, n / 4, n / 2, n - 2] {
                let mut actual = input.clone();
                select_with_budget(&mut actual, k, 0);
                assert_eq!(actual[k], sorted[k]);
                assert!(actual[..k].iter().all(|x| *x <= actual[k]));
                assert!(actual[k + 1..].iter().all(|x| *x >= actual[k]));
                actual.sort_unstable();
                assert_eq!(actual, sorted);
            }
        }
    }

    #[cfg(feature = "bench-instrumentation")]
    #[test]
    fn equal_partition_skips_already_greater_values() {
        for n in [1, 2, 65, 128] {
            let mut values = vec![1u8; n];
            values[0] = 0;
            instrumentation::reset();
            assert_eq!(partition_equal::<_, true>(&mut values, 0), (0, 1));
            let counts = instrumentation::snapshot();
            assert_eq!(counts.elements_scanned, (n - 1) as u64);
            // Only the two pivot swaps; the greater suffix stays in place.
            assert_eq!(counts.bytes_moved, 6);
            assert_eq!(values[0], 0);
            assert!(values[1..].iter().all(|v| *v == 1));
        }

        let mut values: Vec<_> = (0..257).map(|i| u8::from(i % 4 != 0)).collect();
        instrumentation::reset();
        assert_eq!(partition_equal::<_, true>(&mut values, 0), (0, 65));
        let counts = instrumentation::snapshot();
        assert_eq!(counts.elements_scanned, 256);
        // At most one exchange for each remaining equal element.
        assert!(counts.bytes_moved <= 6 + 3 * 64);
        assert!(values[..65].iter().all(|v| *v == 0));
        assert!(values[65..].iter().all(|v| *v == 1));
    }

    #[cfg(feature = "bench-instrumentation")]
    #[test]
    fn exhausted_budget_counts_an_emergency_fallback() {
        instrumentation::reset();
        let mut values: Vec<_> = (0..128).rev().collect();
        select_with_budget(&mut values, 64, 0);
        let counts = instrumentation::snapshot();
        assert_eq!(values[64], 64);
        assert_eq!(counts.budget_fallbacks, 1);
        assert_eq!(counts.small_slice_dispatches, 0);
        assert!(counts.elements_scanned > 0);
        assert!(counts.bytes_moved > 0);
    }

    #[test]
    fn partitions_preserve_values_for_every_pivot() {
        for n in 1..=7u32 {
            for mut code in 0..3usize.pow(n) {
                let input: Vec<_> = (0..n)
                    .map(|_| {
                        let value = code % 3;
                        code /= 3;
                        value
                    })
                    .collect();
                let mut sorted = input.clone();
                sorted.sort_unstable();
                for p in 0..input.len() {
                    let mut actual = input.clone();
                    let k = partition(&mut actual, p);
                    assert_eq!(actual[k], input[p]);
                    assert!(actual[..k].iter().all(|x| *x < actual[k]));
                    assert!(actual[k + 1..].iter().all(|x| *x >= actual[k]));
                    actual.sort_unstable();
                    assert_eq!(actual, sorted);

                    for partition_equal in [partition_equal::<_, false>, partition_equal::<_, true>]
                    {
                        let mut actual = input.clone();
                        let (lo, hi) = partition_equal(&mut actual, p);
                        assert!(lo < hi);
                        assert!(actual[..lo].iter().all(|x| *x < input[p]));
                        assert!(actual[lo..hi].iter().all(|x| *x == input[p]));
                        assert!(actual[hi..].iter().all(|x| *x > input[p]));
                        actual.sort_unstable();
                        assert_eq!(actual, sorted);
                    }
                }
            }
        }
    }
}
