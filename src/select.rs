//! Sampled quickselect with a deterministic linear-time fallback.

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
        let n = a.len();
        if n == 1 {
            return;
        }
        // Extreme ranks need only a scan, including after narrowing the slice.
        if k == 0 {
            let p = (1..n).fold(0, |p, i| if a[i] < a[p] { i } else { p });
            a.swap(0, p);
            return;
        }
        if k == n - 1 {
            let p = (1..n).fold(0, |p, i| if a[i] > a[p] { i } else { p });
            a.swap(n - 1, p);
            return;
        }
        // Keep the existing selector for small slices. Charge each sampled pass
        // for its whole active slice, rather than limiting recursion depth:
        // at most 4 * original_len partition elements are processed before
        // the O(n) fallback. Pivot sampling is also at most linear per pass.
        if n <= 64 || budget < n {
            quickselect(repeated_step3, a, k);
            return;
        }
        budget -= n;

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
        if a[x] == a[y] || a[y] == a[z] || a[x] == a[z] {
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
    a.swap(0, pivot);
    let (pivot, tail) = a.split_first_mut().unwrap();
    let mut count = 0;
    if size_of::<T>() <= 8 {
        let mut i = 0;
        while i + 1 < tail.len() {
            let first = tail[i] < *pivot;
            let second = tail[i + 1] < *pivot;
            // count <= i, so the first swap cannot change tail[i + 1].
            // Classifying the pair before moving it shortens the dependency chain.
            tail.swap(i, count);
            count += usize::from(first);
            tail.swap(i + 1, count);
            count += usize::from(second);
            i += 2;
        }
        if i < tail.len() {
            let less = tail[i] < *pivot;
            tail.swap(i, count);
            count += usize::from(less);
        }
    } else {
        for i in 0..tail.len() {
            let less = tail[i] < *pivot;
            // count <= i. When less is false, both exchanged values are >= pivot;
            // otherwise this extends the strictly smaller prefix by one element.
            tail.swap(i, count);
            count += usize::from(less);
        }
    }
    a.swap(0, count);
    count
}

// Return the half-open range containing all values equal to the pivot.
fn partition_equal<T: Ord>(a: &mut [T], pivot: usize) -> (usize, usize) {
    a.swap(0, pivot);
    let (pivot, tail) = a.split_first_mut().unwrap();
    let (mut lo, mut i, mut hi) = (0, 0, tail.len());
    while i < hi {
        match tail[i].cmp(pivot) {
            Ordering::Less => {
                tail.swap(lo, i);
                lo += 1;
                i += 1;
            }
            Ordering::Equal => i += 1,
            Ordering::Greater => {
                hi -= 1;
                tail.swap(i, hi);
            }
        }
    }
    a.swap(0, lo);
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
