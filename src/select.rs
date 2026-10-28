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
        // at most 4 * original_len elements are scanned before the O(n) fallback.
        if n <= 64 || budget < n {
            quickselect(repeated_step3, a, k);
            return;
        }
        budget -= n;

        let step = n / 9;
        let x = median3_index(a, 0, step, 2 * step);
        let y = median3_index(a, 3 * step, 4 * step, 5 * step);
        let z = median3_index(a, 6 * step, 7 * step, 8 * step);
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

        // Unconditional swaps remove the unpredictable comparison branch for
        // small elements. For large elements, Hoare partitioning moves less data.
        let p = if size_of::<T>() <= 16 {
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
    for i in 0..tail.len() {
        let less = tail[i] < *pivot;
        // count <= i. When less is false, both exchanged values are >= pivot;
        // otherwise this extends the strictly smaller prefix by one element.
        tail.swap(i, count);
        count += usize::from(less);
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

#[cfg(test)]
mod tests {
    use super::*;

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
