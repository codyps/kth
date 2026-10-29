//! Benchmark-only selection experiments. Production remains in src/select.rs.
//! Reference algorithms and sources are documented in benchmarks/EXPERIMENTS.md.

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
            if n <= 64 && (MODE == 1 || MODE == 2 || MODE == 13) {
                if MODE == 13 {
                    small_select::<T, true>(a, k, 16);
                } else {
                    small_select::<T, false>(a, k, if MODE == 1 { 16 } else { 32 });
                }
            } else if MODE == 4 {
                quickselect(central_ninthers, a, k);
            } else {
                quickselect(repeated_step3, a, k);
            }
            return;
        }
        budget -= n;

        let sampling_phase = instrumentation::phase(Phase::Sampling);
        if MODE == 8 {
            let (left, right, equal) = ses_partition(a, k);
            drop(sampling_phase);
            if k < left {
                a = &mut a[..left];
            } else if k > right {
                a = &mut a[right + 1..];
                k -= right + 1;
            } else if k == left || k == right || equal {
                return;
            } else {
                a = &mut a[left + 1..right];
                k -= left + 1;
            }
            continue;
        }
        let (pivot, repeated) =
            if (MODE == 3 || MODE == 11 || MODE == 12 || (MODE == 10 && size_of::<T>() > 1))
                && n > 600
            {
                if MODE == 11 || MODE == 12 {
                    floyd_rivest_pivot::<T, true>(a, k)
                } else {
                    floyd_rivest_pivot::<T, false>(a, k)
                }
            } else {
                choose_pivot(a, k)
            };
        drop(sampling_phase);
        let ancestor_equal =
            (MODE == 6 || MODE == 7) && ancestor.is_some_and(|lower| a[pivot] == *lower);
        if ancestor_equal || (repeated && MODE != 6) {
            // A repeated sample suggests many duplicates. Discard the whole
            // equal band instead of repeatedly selecting among equal elements.
            // For small elements, a low-rank pivot often leaves a mostly-greater
            // suffix. Skip greater/greater swaps there. Keeping the original loop
            // for larger types also keeps their partition driver small.
            let (lo, hi) = if ancestor_equal {
                (0, partition_min_equal(a, pivot))
            } else if size_of::<T>() <= 8 && k <= n / 8 {
                partition_equal::<_, true>(a, pivot)
            } else {
                partition_equal::<_, false>(a, pivot)
            };
            if k < lo {
                a = &mut a[..lo];
            } else if k >= hi {
                let (discarded, remaining) = a.split_at_mut(hi);
                if MODE == 6 || MODE == 7 {
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
        let p = if MODE == 5 || MODE == 9 || ((MODE == 10 || MODE == 12) && size_of::<T>() > 32) {
            if MODE == 5 {
                block_partition::<T, 64>(a, pivot)
            } else {
                block_partition::<T, 128>(a, pivot)
            }
        } else if size_of::<T>() <= 32 {
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
            let (discarded, remaining) = a.split_at_mut(p + 1);
            if MODE == 6 || MODE == 7 {
                ancestor = Some(&discarded[p]);
            }
            a = remaining;
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

pub fn control<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 0>(a, k);
}
pub fn small16<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 1>(a, k);
}
pub fn small32<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 2>(a, k);
}
pub fn fr_window<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 3>(a, k);
}
pub fn expand<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 4>(a, k);
}
pub fn block64<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 5>(a, k);
}
pub fn ancestor<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 6>(a, k);
}
pub fn ancestor_sample<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 7>(a, k);
}
pub fn sesquick<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 8>(a, k);
}
pub fn block128<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 9>(a, k);
}

fn small_select<T: Ord, const NETWORK: bool>(mut a: &mut [T], mut k: usize, cutoff: usize) {
    loop {
        if a.len() <= cutoff {
            if NETWORK {
                network_sort(a);
                return;
            }
            for i in 1..a.len() {
                let mut j = i;
                while j > 0 {
                    instrumentation::scanned(1);
                    if a[j - 1] <= a[j] {
                        break;
                    }
                    instrumentation::swap(a, j - 1, j);
                    j -= 1;
                }
            }
            return;
        }
        let pivot = median3_index(a, 0, a.len() / 2, a.len() - 1);
        let (lo, hi) = partition_equal::<T, false>(a, pivot);
        if k < lo {
            a = &mut a[..lo];
        } else if k >= hi {
            a = &mut a[hi..];
            k -= hi;
        } else {
            return;
        }
    }
}

// Floyd–Rivest's contiguous sample-window rule, with recursive selection in
// that window. The existing partition kernels and budget remain in use.
// No random shuffle: timings include structured-input sensitivity explicitly.
#[inline(never)]
fn floyd_rivest_pivot<T: Ord, const REPEAT9: bool>(a: &mut [T], k: usize) -> (usize, bool) {
    let n = a.len();
    let repeat9 = REPEAT9 && choose_pivot(a, k).1;
    let m = (k + 1) as f64;
    let z = (n as f64).ln();
    let s = 0.5 * (2.0 * z / 3.0).exp();
    let deviation = 0.5
        * (z * s * (n as f64 - s) / n as f64).sqrt()
        * if m < n as f64 / 2.0 { -1.0 } else { 1.0 };
    let left = ((k as f64 - m * s / n as f64 + deviation).floor().max(0.0) as usize).min(k);
    let right = ((k as f64 + (n as f64 - m) * s / n as f64 + deviation).floor() as usize)
        .max(k)
        .min(n - 1);
    if right - left + 1 < n {
        if REPEAT9 {
            select::<T, 11>(&mut a[left..=right], k - left);
        } else {
            select::<T, 3>(&mut a[left..=right], k - left);
        }
    }
    (
        k,
        if REPEAT9 {
            repeat9
        } else {
            a[k] == a[k - 1] || a[k] == a[k + 1]
        },
    )
}

// The lower bound lives in a discarded slice. Equality proves that no active
// value is smaller, so one equality test suffices instead of general cmp.
fn partition_min_equal<T: Ord>(a: &mut [T], pivot: usize) -> usize {
    instrumentation::partition_pass();
    instrumentation::scanned(a.len() - 1);
    instrumentation::swap(a, 0, pivot);
    let (pivot, tail) = a.split_first_mut().unwrap();
    let mut count = 0;
    for i in 0..tail.len() {
        if tail[i] == *pivot {
            instrumentation::swap(tail, count, i);
            count += 1;
        }
    }
    count + 1
}

fn block_partition<T: Ord, const B: usize>(a: &mut [T], pivot: usize) -> usize {
    instrumentation::partition_pass();
    instrumentation::swap(a, 0, pivot);
    let (pivot, tail) = a.split_first_mut().unwrap();
    let (mut left, mut right) = (0, tail.len());
    let (mut left_offsets, mut right_offsets) = ([0usize; B], [0usize; B]);
    let (mut nl, mut nr, mut il, mut ir) = (0, 0, 0, 0);
    while right - left >= 2 * B {
        if il == nl {
            nl = 0;
            il = 0;
            instrumentation::scanned(B);
            for i in 0..B {
                left_offsets[nl] = i;
                nl += usize::from(tail[left + i] >= *pivot);
            }
        }
        if ir == nr {
            nr = 0;
            ir = 0;
            instrumentation::scanned(B);
            for i in 0..B {
                right_offsets[nr] = i;
                nr += usize::from(tail[right - 1 - i] < *pivot);
            }
        }
        let pairs = (nl - il).min(nr - ir);
        for j in 0..pairs {
            instrumentation::swap(
                tail,
                left + left_offsets[il + j],
                right - 1 - right_offsets[ir + j],
            );
        }
        il += pairs;
        ir += pairs;
        if il == nl {
            left += B;
        }
        if ir == nr {
            right -= B;
        }
    }
    // Finish the unresolved interval, including any partially consumed block.
    while left < right {
        instrumentation::scanned(1);
        if tail[left] < *pivot {
            left += 1;
            continue;
        }
        while left < right {
            right -= 1;
            if left == right {
                break;
            }
            instrumentation::scanned(1);
            if tail[right] < *pivot {
                instrumentation::swap(tail, left, right);
                left += 1;
                break;
            }
        }
    }
    instrumentation::swap(a, 0, left);
    left
}

// Nine disjoint positions per group: four in the left region, one in the
// central ninth, four in the right. Multiplying the outer offsets by four is
// essential; the old commented outline in quickselect.rs overlaps its groups.
fn central_ninthers<T: Ord>(a: &mut [T]) -> usize {
    let f = a.len() / 9;
    if f == 0 {
        return hoare_partition(a, a.len() / 2);
    }
    for j in 0..f {
        let (l, m, r) = (4 * j, 4 * f + j, 5 * f + 4 * j);
        instrumentation::scanned(9);
        kth::quickselect::ninther(a, l, l + 1, l + 2, l + 3, m, r, r + 1, r + 2, r + 3);
    }
    quickselect(central_ninthers, &mut a[4 * f..5 * f], f / 2);
    expand_partition(a, 4 * f, 4 * f + f / 2, 5 * f)
}

// Extend an already partitioned central sample. Symmetric mistakes cost one
// exchange; remaining asymmetric mistakes rotate across the pivot in two swaps.
// All values remain initialized even if a comparison panics.
fn expand_partition<T: Ord>(a: &mut [T], start: usize, mut p: usize, end: usize) -> usize {
    instrumentation::partition_pass();
    let (mut left, mut right) = (0, a.len());
    while left < start && right > end {
        while left < start {
            instrumentation::scanned(1);
            if a[left] > a[p] {
                break;
            }
            left += 1;
        }
        if left == start {
            break;
        }
        while right > end {
            instrumentation::scanned(1);
            if a[right - 1] < a[p] {
                break;
            }
            right -= 1;
        }
        if right == end {
            break;
        }
        right -= 1;
        instrumentation::swap(a, left, right);
        left += 1;
    }
    if left < start {
        for i in (left..start).rev() {
            instrumentation::scanned(1);
            if a[i] > a[p] {
                instrumentation::swap(a, i, p - 1);
                instrumentation::swap(a, p - 1, p);
                p -= 1;
            }
        }
    } else {
        for i in end..right {
            instrumentation::scanned(1);
            if a[i] < a[p] {
                instrumentation::swap(a, i, p + 1);
                instrumentation::swap(a, p, p + 1);
                p += 1;
            }
        }
    }
    p
}

// Sesquickselect with two sample values, nu ~= 0.265717, and YBB partitioning
// in the central rank region. Equal pivots use the existing equal-band kernel.
fn ses_partition<T: Ord>(a: &mut [T], k: usize) -> (usize, usize, bool) {
    let n = a.len();
    instrumentation::swap(a, 0, n / 3);
    instrumentation::swap(a, n - 1, 2 * n / 3);
    if a[0] > a[n - 1] {
        instrumentation::swap(a, 0, n - 1);
    }
    let equal = a[0] == a[n - 1];
    let _phase = instrumentation::phase(Phase::Partition);
    if equal {
        let (lo, hi) = partition_equal::<T, false>(a, 0);
        return (lo, hi - 1, true);
    }
    let cutoff = n / 1_000_000 * 265_717 + (n % 1_000_000) * 265_717 / 1_000_000;
    if k < cutoff || n - 1 - k < cutoff {
        let pivot = if k < cutoff { 0 } else { n - 1 };
        let p = if size_of::<T>() <= 32 {
            partition(a, pivot)
        } else {
            hoare_partition(a, pivot)
        };
        return (p, p, false);
    }
    instrumentation::partition_pass();
    let (mut lo, mut i, mut hi) = (1, 1, n - 2);
    while i <= hi {
        instrumentation::scanned(1);
        if a[i] < a[0] {
            instrumentation::swap(a, i, lo);
            lo += 1;
        } else if a[i] > a[n - 1] {
            while i < hi {
                instrumentation::scanned(1);
                if a[hi] <= a[n - 1] {
                    break;
                }
                hi -= 1;
            }
            instrumentation::swap(a, i, hi);
            hi -= 1;
            if a[i] < a[0] {
                instrumentation::swap(a, i, lo);
                lo += 1;
            }
        }
        i += 1;
    }
    lo -= 1;
    hi += 1;
    instrumentation::swap(a, 0, lo);
    instrumentation::swap(a, n - 1, hi);
    (lo, hi, false)
}

pub fn fr_block<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 10>(a, k);
}
pub fn ninther_all<T: Ord>(a: &mut [T], k: usize) {
    quickselect(central_ninthers, a, k);
}

pub fn fr_sample<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 11>(a, k);
}
pub fn fr_sample_block<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 12>(a, k);
}

pub fn network16<T: Ord>(a: &mut [T], k: usize) {
    select::<T, 13>(a, k);
}

// Batcher odd-even merge network. All compare-exchanges point upward, so
// indices beyond the slice can be treated as +infinity and omitted safely.
const fn network_pairs() -> [(usize, usize); 63] {
    let mut pairs = [(0, 0); 63];
    let mut count = 0;
    let mut p = 1;
    while p < 16 {
        let mut k = p;
        while k > 0 {
            let mut j = k % p;
            while j + k < 16 {
                let mut i = 0;
                while i < k {
                    if (i + j) / (2 * p) == (i + j + k) / (2 * p) {
                        pairs[count] = (i + j, i + j + k);
                        count += 1;
                    }
                    i += 1;
                }
                j += 2 * k;
            }
            k /= 2;
        }
        p *= 2;
    }
    assert!(count == 63);
    pairs
}
fn network_sort<T: Ord>(a: &mut [T]) {
    debug_assert!(a.len() <= 16);
    for (left, right) in network_pairs() {
        if right < a.len() {
            instrumentation::scanned(2);
            if a[left] > a[right] {
                instrumentation::swap(a, left, right);
            }
        }
    }
}
