//! Benchmark-only robust growing samples and disorder-gated block partitioning.

use core::{cmp::Ordering, mem::size_of};
use kth::instrumentation::{self, Phase};
use kth::quickselect::{hoare_partition, median3_index, quickselect, repeated_step3};

fn select<T: Ord, const MODE: u8>(a: &mut [T], k: usize) {
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
    select_with_budget::<T, MODE>(a, k, a.len().saturating_mul(4));
}

fn select_with_budget<T: Ord, const MODE: u8>(mut a: &mut [T], mut k: usize, mut budget: usize) {
    let mut ancestor: Option<&T> = None;
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
        let (pivot, repeated) = if MODE == 13 {
            verified_growing_pivot::<T, true, 8>(a, k)
        } else if MODE == 14 {
            verified_growing_pivot::<T, true, 32>(a, k)
        } else if MODE == 11 {
            verified_growing_pivot::<T, true, 16>(a, k)
        } else if MODE == 12 {
            verified_growing_pivot::<T, false, 16>(a, k)
        } else if MODE == 8 {
            guarded_pivot_impl::<T, false, true>(a, k)
        } else if MODE == 9 {
            guarded_pivot_impl::<T, true, true>(a, k)
        } else if MODE == 6 || MODE == 7 {
            choose_pivot_impl::<T, true>(a, k)
        } else if MODE == 5 {
            guarded_pivot(a, k)
        } else if (MODE == 1 || MODE == 3 || MODE == 4)
            && n >= 4096
            && (MODE == 4 || (k > n / 8 && n - 1 - k > n / 8))
        {
            stratified_pivot::<T, false>(a, k)
        } else {
            choose_pivot(a, k)
        };
        let repeated = repeated
            && (MODE != 10
                || n < 1024
                || k <= n / 8
                || n - 1 - k <= n / 8
                || confirms_repeat::<T, 16>(a, pivot));
        drop(sampling_phase);
        let ancestor_equal =
            (MODE == 7 || MODE == 9) && ancestor.is_some_and(|lower| a[pivot] == *lower);
        if repeated || ancestor_equal {
            // A repeated sample suggests many duplicates. Discard the whole
            // equal band instead of repeatedly selecting among equal elements.
            // For small elements, a low-rank pivot often leaves a mostly-greater
            // suffix. Skip greater/greater swaps there. Keeping the original loop
            // for larger types also keeps their partition driver small.
            let (lo, hi) = if ancestor_equal {
                (0, super::experiments::partition_min_equal(a, pivot))
            } else if size_of::<T>() <= 8 && k <= n / 8 {
                partition_equal::<_, true>(a, pivot)
            } else {
                partition_equal::<_, false>(a, pivot)
            };
            if k < lo {
                a = &mut a[..lo];
            } else if k >= hi {
                let (discarded, remaining) = a.split_at_mut(hi);
                if MODE == 7 || MODE == 9 {
                    ancestor = Some(&discarded[hi - 1]);
                }
                a = remaining;
                k -= hi;
            } else {
                return;
            }
            continue;
        }

        // Unconditional swaps avoid an unpredictable branch for moderately sized
        // elements, including String. For larger elements, Hoare moves less data.
        let p = if size_of::<T>() <= 32 {
            if (MODE == 8 || MODE == 9)
                && size_of::<T>() > 8
                && n >= 16384
                && worth_blocking(a, pivot)
            {
                super::cyclic_baseline::partition(a, pivot)
            } else {
                partition(a, pivot)
            }
        } else {
            let block_min = if MODE == 5 || MODE == 8 || MODE == 9 || MODE >= 11 {
                16384
            } else {
                1024
            };
            if (MODE == 2
                || MODE == 3
                || MODE == 4
                || MODE == 5
                || MODE == 8
                || MODE == 9
                || MODE >= 11)
                && n >= block_min
                && if MODE >= 11 {
                    conservative_disorder(a, pivot)
                } else {
                    worth_blocking(a, pivot)
                }
            {
                super::experiments::block_partition::<T, 128>(a, pivot)
            } else {
                hoare_partition(a, pivot)
            }
        };
        if p == k {
            return;
        }
        if p > k {
            a = &mut a[..p];
        } else {
            let (discarded, remaining) = a.split_at_mut(p + 1);
            if MODE == 7 || MODE == 9 {
                ancestor = Some(&discarded[p]);
            }
            a = remaining;
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
    choose_pivot_impl::<T, false>(a, k)
}

#[inline(always)]
fn choose_pivot_impl<T: Ord, const STRICT: bool>(a: &[T], k: usize) -> (usize, bool) {
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
    let repeated = if STRICT {
        a[x] == a[y] && a[y] == a[z]
    } else {
        a[x] == a[y] || a[y] == a[z] || a[x] == a[z]
    };
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

pub fn adaptive_control<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 0>(a, k);
}

pub fn stratified_mid<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 1>(a, k);
}

pub fn adaptive_block<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 2>(a, k);
}

pub fn stratified_block<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 3>(a, k);
}

pub fn stratified_all<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 4>(a, k);
}

// Deterministic jitter within disjoint strata avoids a fixed stride aliasing
// periodic inputs. Forward gathering cannot overwrite a future stratum.
#[inline(never)]
fn stratified_pivot<T: Ord, const STRICT: bool>(a: &mut [T], k: usize) -> (usize, bool) {
    let repeated = choose_pivot_impl::<T, STRICT>(a, k).1;
    let n = a.len();
    let count = (2 * n.isqrt()).min(1024);
    let width = n / count;
    let extra = n % count;
    let mut start = 0;
    let mut state = (n as u64) ^ 0x243f6a8885a308d3;
    for i in 0..count {
        let length = width + usize::from(i < extra);
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let index = start + ((state >> 32) as usize) % length;
        instrumentation::swap(a, i, index);
        start += length;
    }
    let mut rank = ((k as u128) * (count as u128) / (n as u128)) as usize;
    let bias = count.isqrt() / 2;
    if rank < count / 4 {
        rank += bias;
    } else if rank > count - count / 4 {
        rank = rank.saturating_sub(bias);
    }
    rank = rank.clamp(1, count - 2);
    kth::partition_by_kth(&mut a[..count], rank);
    (rank, repeated)
}

// A cheap classifier for predictable distributions. Monotonic inputs produce
// one transition, organ-pipe inputs two, and random median splits many more.
#[inline(never)]
fn worth_blocking<T: Ord>(a: &[T], pivot: usize) -> bool {
    let _phase = instrumentation::phase(Phase::Sampling);
    let step = a.len() / 17;
    let mut last = a[step] < a[pivot];
    let mut changes = 0;
    for i in 2..=16 {
        let next = a[i * step] < a[pivot];
        changes += usize::from(next != last);
        last = next;
    }
    changes >= 4
}

pub fn guarded<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 5>(a, k);
}
#[inline(never)]
fn guarded_pivot<T: Ord>(a: &mut [T], k: usize) -> (usize, bool) {
    guarded_pivot_impl::<T, false, false>(a, k)
}

#[inline(never)]
fn guarded_pivot_impl<T: Ord, const STRICT: bool, const INCLUDE_MID: bool>(
    a: &mut [T],
    k: usize,
) -> (usize, bool) {
    let base = choose_pivot_impl::<T, STRICT>(a, k);
    if a.len() >= 16384
        && (INCLUDE_MID || !(size_of::<T>() > 16 && size_of::<T>() <= 32))
        && !base.1
        && worth_blocking(a, base.0)
    {
        stratified_pivot::<T, STRICT>(a, k)
    } else {
        base
    }
}

pub fn strict_duplicates<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 6>(a, k);
}
pub fn bounded_duplicates<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 7>(a, k);
}

pub fn guarded_cyclic<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 8>(a, k);
}
pub fn combined<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 9>(a, k);
}

// Verify a weak sample hint against jittered probes, rather than requiring all
// three representatives to match. A frequent pivot should be seen quickly;
// accidental equal representatives in high-entropy byte data usually are not.
#[inline(never)]
fn confirms_repeat<T: Ord, const PROBES: usize>(a: &[T], pivot: usize) -> bool {
    let mut state = (a.len() as u64) ^ (pivot as u64).rotate_left(17) ^ 0x13198a2e03707344;
    let step = a.len() / PROBES;
    for i in 0..PROBES {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let start = i * step;
        let width = if i == PROBES - 1 {
            a.len() - start
        } else {
            step
        };
        let index = start + ((state >> 32) as usize) % width;
        if a[index] == a[pivot] {
            return true;
        }
    }
    false
}

#[inline(never)]
fn conservative_disorder<T: Ord>(a: &[T], pivot: usize) -> bool {
    let step = a.len() / 17;
    let first = a[step].cmp(&a[pivot]);
    if first == Ordering::Equal {
        return false;
    }
    let mut last = first == Ordering::Less;
    let mut changes = 0;
    for i in 2..=16 {
        let order = a[i * step].cmp(&a[pivot]);
        if order == Ordering::Equal {
            return false;
        }
        let next = order == Ordering::Less;
        changes += usize::from(next != last);
        last = next;
    }
    changes >= 4
}

#[inline(never)]
fn verified_growing_pivot<T: Ord, const VERIFY: bool, const PROBES: usize>(
    a: &mut [T],
    k: usize,
) -> (usize, bool) {
    let (pivot, mut repeated) = choose_pivot(a, k);
    let n = a.len();
    if VERIFY && repeated && n >= 1024 && k > n / 8 && n - 1 - k > n / 8 {
        repeated = confirms_repeat::<T, PROBES>(a, pivot);
    }
    if n >= 16384
        && !(size_of::<T>() > 16 && size_of::<T>() <= 32)
        && (size_of::<T>() > 32 || (k > n / 8 && n - 1 - k > n / 8))
        && !repeated
        && conservative_disorder(a, pivot)
    {
        (stratified_pivot::<T, false>(a, k).0, false)
    } else {
        (pivot, repeated)
    }
}

pub fn verified_duplicates<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 10>(a, k);
}
pub fn verified_guarded<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 11>(a, k);
}
pub fn verified_control<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 12>(a, k);
}

pub fn verified8<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 13>(a, k);
}
pub fn verified32<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 14>(a, k);
}
