// Frozen cyclic selector from the cyclic-partition experiment.
// Uses the pre-rank-aware pivots and remains a benchmark variant only.

use core::{
    cmp::Ordering,
    mem::{self, size_of, ManuallyDrop},
    ptr,
};
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

        // Branchless cyclic moves avoid an unpredictable branch for moderately
        // sized elements, including String. For larger elements, Hoare moves less data.
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

pub(crate) fn partition<T: Ord>(a: &mut [T], pivot: usize) -> usize {
    instrumentation::partition_pass();
    instrumentation::scanned(a.len() - 1);
    instrumentation::swap(a, 0, pivot);
    let (pivot, tail) = a.split_first_mut().unwrap();
    if tail.is_empty() {
        return 0;
    }

    // Branchless Lomuto with a cyclic permutation, following Bergdoll and Peters:
    // https://github.com/Voultapher/sort-research-rs/blob/main/writeup/lomcyc_partition/text.md
    // Save the first element, then carry the hole through the slice instead of
    // completing a separate swap on every iteration.
    let base = tail.as_mut_ptr();
    let mut count = 0;
    // SAFETY: tail is nonempty. At iteration i, count < i < tail.len(),
    // so left and right are distinct in-bounds elements. The previous hole is
    // also in bounds; ptr::copy permits it to overlap left. Each iteration fills
    // that hole and creates a new one at right. No comparison observes a hole.
    // The guard restores the saved element if any comparison unwinds.
    instrumentation::moved::<T>(1); // Save the first tail element.
    unsafe {
        let mut gap = PartitionGap {
            value: ManuallyDrop::new(ptr::read(base)),
            pos: base,
        };
        let mut move_element = |i: usize| {
            let right = base.add(i);
            let less = *right < *pivot;
            let left = base.add(count);
            instrumentation::moved::<T>(2);
            ptr::copy(left, gap.pos, 1);
            ptr::copy_nonoverlapping(right, left, 1);
            gap.pos = right;
            count += usize::from(less);
        };
        let mut i = 1;
        if size_of::<T>() <= 8 {
            while i + 1 < tail.len() {
                move_element(i);
                move_element(i + 1);
                i += 2;
            }
        }
        while i < tail.len() {
            move_element(i);
            i += 1;
        }

        // Compare the saved value in its actual storage so interior mutations
        // made by Ord are preserved, including when this comparison panics.
        // count <= tail.len() - 1, and the saved value cannot alias the slice.
        let less = *gap.value < *pivot;
        let left = base.add(count);
        instrumentation::moved::<T>(2);
        ptr::copy(left, gap.pos, 1);
        ptr::copy_nonoverlapping(&*gap.value, left, 1);
        count += usize::from(less);
        // Ownership of the saved value has returned to the slice.
        mem::forget(gap);
    }
    instrumentation::swap(a, 0, count);
    count
}

struct PartitionGap<T> {
    value: ManuallyDrop<T>,
    pos: *mut T,
}

impl<T> Drop for PartitionGap<T> {
    fn drop(&mut self) {
        // SAFETY: pos is the current in-bounds hole in the partition. The saved
        // value is owned by this guard and is moved back exactly once on unwind.
        unsafe { ptr::copy_nonoverlapping(&*self.value, self.pos, 1) };
    }
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

#[cfg(test)]
#[allow(unused_imports)] // Also compiled by Criterion's harness-free bench targets.
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn cyclic_partition_preserves_ownership_and_comparison_mutations() {
        struct State {
            calls: Cell<usize>,
            panic_at: usize,
            visits: Vec<Cell<usize>>,
            drops: Vec<Cell<usize>>,
        }
        struct Item {
            id: usize,
            visits: Cell<usize>,
            state: Rc<State>,
        }
        impl PartialEq for Item {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id
            }
        }
        impl Eq for Item {}
        impl Ord for Item {
            fn cmp(&self, other: &Self) -> Ordering {
                for item in [self, other] {
                    item.visits.set(item.visits.get() + 1);
                    let expected = &item.state.visits[item.id];
                    expected.set(expected.get() + 1);
                }
                let calls = self.state.calls.get();
                self.state.calls.set(calls + 1);
                assert_ne!(calls, self.state.panic_at, "intentional comparison panic");
                self.id.cmp(&other.id)
            }
        }
        impl PartialOrd for Item {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Drop for Item {
            fn drop(&mut self) {
                let drops = &self.state.drops[self.id];
                drops.set(drops.get() + 1);
            }
        }
        fn exercise<T: Ord + AsRef<Item>>(wrap: impl Fn(Item) -> T) {
            for n in 1..=10 {
                for pivot in 0..n {
                    // There are n - 1 comparisons, including the final comparison
                    // against the saved element. n - 1 also tests normal return.
                    for panic_at in 0..n {
                        let state = Rc::new(State {
                            calls: Cell::new(0),
                            panic_at,
                            visits: (0..n).map(|_| Cell::new(0)).collect(),
                            drops: (0..n).map(|_| Cell::new(0)).collect(),
                        });
                        let mut values: Vec<_> = (0..n)
                            .rev()
                            .map(|id| {
                                wrap(Item {
                                    id,
                                    visits: Cell::new(0),
                                    state: state.clone(),
                                })
                            })
                            .collect();
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            partition(&mut values, pivot)
                        }));
                        assert_eq!(result.is_err(), panic_at < n - 1);
                        let mut ids: Vec<_> = values.iter().map(|v| v.as_ref().id).collect();
                        ids.sort_unstable();
                        assert_eq!(ids, (0..n).collect::<Vec<_>>());
                        for value in &values {
                            let item = value.as_ref();
                            assert_eq!(item.visits.get(), state.visits[item.id].get());
                        }
                        assert!(state.drops.iter().all(|n| n.get() == 0));
                        drop(values);
                        assert!(state.drops.iter().all(|n| n.get() == 1));
                    }
                }
            }
        }
        impl AsRef<Item> for Item {
            fn as_ref(&self) -> &Item {
                self
            }
        }
        // Exercise both the ordinary loop and the unrolled small-element loop.
        exercise(|item| item);
        exercise(Box::new);
    }

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

    #[cfg(feature = "instrumentation")]
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
